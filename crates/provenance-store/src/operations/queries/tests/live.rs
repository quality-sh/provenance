//! One test per live word: change that part of the answer alone and the
//! answer moves while the stamp's serial and digest stand still, since
//! the stamp never covered it.

use super::comparison::requests;
use super::comparison::test_stores::{self, git_commit, TestStore};
use crate::cache::tests::fixtures::{append_record, create_rule_of, sid};
use crate::operations::queries::{self, stale};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader;
use crate::state_store::{CreateRuleInput, StateStore};
use provenance_core::protocol::{ImpactQuery, StaleQuery, Stamp, SDK_PROTOCOL_VERSION};
use provenance_core::{RuleSeverity, RuleStatus, ScopeId, SUPPORTED_SCHEMA_VERSION};
use serde_json::json;

fn store_with_rule() -> TestStore {
    let store = test_stores::seeded_queries();
    create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_overtime",
        "req_overtime",
    );
    store
}

/// A rule whose source document is a repository path, so the diff gate
/// has a site to report.
fn create_rule_citing(store: &StateStore, scope: &ScopeId, id: &str, document: &str) {
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: sid(id),
            name: None,
            description: None,
            requirement_ids: vec![sid("req_overtime")],
            resolution_ids: Vec::new(),
            statement: format!("{id} statement"),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: Some(document.into()),
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn assert_same_revision(first: &Stamp, second: &Stamp) {
    assert_eq!(second.serial, first.serial, "the serial stood still");
    assert_eq!(second.digest, first.digest, "the digest stood still");
    assert_eq!(second.instance_id, first.instance_id);
}

#[tokio::test]
async fn a_scanner_annotation_moves_impact_and_not_the_stamp() {
    let store = store_with_rule();
    let request = || ImpactQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: "req_overtime".into(),
        node_type: None,
        include_retired: false,
        limit: 50,
    };
    let sites = |answer: &provenance_core::protocol::ImpactResult| -> Vec<String> {
        answer.affected_rules[0]
            .implementations
            .iter()
            .map(|site| site.file.to_string())
            .collect()
    };
    let first = queries::impact(Some(store.root.clone()), &store.scope, request())
        .await
        .unwrap();
    assert_eq!(sites(&first.result), ["src/pay.rs"]);

    std::fs::write(
        store.root.join("src/more.rs"),
        "#[rule(\"rule_overtime\")]\nfn more() {}\n",
    )
    .unwrap();
    let second = queries::impact(Some(store.root.clone()), &store.scope, request())
        .await
        .unwrap();
    assert_eq!(sites(&second.result), ["src/more.rs", "src/pay.rs"]);
    assert_same_revision(&first.stamp, &second.stamp);
    assert_eq!(second.stamp.live, ["scanned_sites"]);
}

#[tokio::test]
async fn an_appended_run_moves_evidence_and_not_the_stamp() {
    let store = store_with_rule();
    let request = || requests::evidence("rule_overtime", None);
    let first = queries::evidence(Some(store.root.clone()), &store.scope, request())
        .await
        .unwrap();
    assert!(first.result.verification_runs.is_empty());
    assert!(first.result.latest_verification_run.is_none());

    append_record(
        &store.layout().verification_runs_path(&store.scope),
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": store.scope.as_str(),
            "id": "run_late",
            "rule_id": "rule_overtime",
            "method": "examples",
            "declared_by": "spec://test",
            "status": "passed",
            "started_at": 7,
        }),
    );
    let second = queries::evidence(Some(store.root.clone()), &store.scope, request())
        .await
        .unwrap();
    assert_eq!(second.result.verification_runs.len(), 1);
    assert_eq!(
        second
            .result
            .latest_verification_run
            .as_ref()
            .map(|run| run.id.as_str()),
        Some("run_late")
    );
    assert_same_revision(&first.stamp, &second.stamp);
    assert!(second.stamp.live.contains(&"verification_runs".to_string()));
}

#[tokio::test]
async fn a_commit_touching_a_bound_file_moves_evidence_and_not_the_stamp() {
    let store = test_stores::seeded_queries();
    let base = store.base_commit.clone().expect("git on the path");
    create_rule_citing(
        &store.state_store(),
        &store.scope,
        "rule_cited",
        "src/pay.rs",
    );
    let request = || requests::evidence("rule_cited", Some(base.clone()));
    let first = queries::evidence(Some(store.root.clone()), &store.scope, request())
        .await
        .unwrap();
    let states = |answer: &provenance_core::protocol::EvidenceResult| -> Vec<String> {
        answer
            .stale
            .as_ref()
            .unwrap()
            .sites
            .iter()
            .map(|site| format!("{:?}", site.state))
            .collect()
    };
    assert!(
        !states(&first.result).is_empty(),
        "the head commit changed the cited file"
    );

    std::fs::remove_file(store.root.join("src/pay.rs")).unwrap();
    git_commit(&store.root, "remove the cited file").expect("a commit");
    let second = queries::evidence(Some(store.root.clone()), &store.scope, request())
        .await
        .unwrap();
    assert_ne!(
        second.result.stale.as_ref().unwrap().head,
        first.result.stale.as_ref().unwrap().head
    );
    assert_ne!(states(&second.result), states(&first.result));
    assert_same_revision(&first.stamp, &second.stamp);
    assert!(second.stamp.live.contains(&"diff".to_string()));
}

/// `stale` reads its graph evidence from canonical shards. Under
/// `annotate_only` a canonical edit reaches the answer and not the stamp.
#[tokio::test]
async fn a_canonical_edit_under_annotate_only_moves_stale_and_not_the_stamp() {
    let store = test_stores::seeded_queries();
    let base = store.base_commit.clone().expect("git on the path");
    crate::cache::catch_up_state(&store.layout()).await.unwrap();
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let stale_under = |base: String| {
        let scope = store.scope.clone();
        reader::answer(&store.root, &store.scope, policy, move |ctx| {
            Box::pin(async move {
                stale::stale(
                    ctx,
                    &scope,
                    StaleQuery {
                        protocol_version: Some(SDK_PROTOCOL_VERSION),
                        base,
                        head: None,
                        rules: Vec::new(),
                        include_retired: false,
                        limit: 50,
                    },
                )
            })
        })
    };
    let first = stale_under(base.clone()).await.unwrap();
    assert!(first.result.sites.is_empty(), "no record names a path yet");

    create_rule_citing(
        &store.state_store(),
        &store.scope,
        "rule_cited",
        "src/pay.rs",
    );
    let second = stale_under(base).await.unwrap();
    assert_eq!(second.result.sites.len(), 1);
    assert_eq!(second.result.sites[0].subject_id, "rule_cited");
    assert_same_revision(&first.stamp, &second.stamp);
    assert_eq!(second.stamp.live, ["canonical", "diff"]);
}
