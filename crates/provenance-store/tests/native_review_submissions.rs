//! Native record writes open their review submission in the same write: on
//! creation, on a content change to a record that was never submitted, on
//! archive parent pruning, and once for a race of two guarded writes.

use provenance_core::{
    CanonicalArtifact, CanonicalArtifactType, DispositionActor, DispositionDecision, IdentityType,
    NodeType, ScopeId, StableId,
};
use provenance_macros::verifies;
use provenance_store::{
    layout::ProvenanceLayout,
    review::{DecideRecordReview, SubmitRecordReview},
    state_store::{StateStore, TypedSpecInput, UpdateRequirementInput, UpdateRuleInput},
    write_error::{WriteError, WriteFailure},
};
use serde_json::{json, Value};

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}"#,
    )
    .unwrap();
    let store = provenance_store::repository::Repository::open(root, "default")
        .unwrap()
        .store();
    (temp, store, ScopeId::new("default").unwrap())
}

fn input<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// The review submissions stored for one record.
fn submissions(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) -> usize {
    let prefix = format!("review:{}:{record}:", kind.as_str());
    store
        .list_proposal_definitions(scope)
        .unwrap()
        .iter()
        .filter(|proposal| proposal.proposal_key.starts_with(&prefix))
        .count()
}

/// The pending submission is bound to the current review revision.
fn assert_pending_at_current(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) {
    let state = store
        .record_decision_state(scope, kind, &id(record))
        .unwrap();
    let pending = state
        .pending
        .unwrap_or_else(|| panic!("{record}: no pending submission"));
    assert_eq!(Some(pending.revision), state.current_revision, "{record}");
}

fn requirement(store: &StateStore, record: &str, parents: &[&str]) {
    store
        .create_requirement(input(json!({
            "scope_id":"default", "id":record, "statement":"The system stores records.",
            "status":"active", "refines":parents, "depends_on":[], "supersedes":[]
        })))
        .unwrap();
}

fn rule(store: &StateStore, record: &str, status: &str, parents: &[&str]) {
    store
        .create_rule(input(json!({
            "scope_id":"default", "id":record, "statement":"The system keeps each record.",
            "status":status, "severity":"medium", "requirement_ids":parents,
            "resolution_ids":[]
        })))
        .unwrap();
}

fn resolution(store: &StateStore, record: &str, status: &str, parents: &[&str]) {
    store
        .create_resolution(input(json!({
            "scope_id":"default", "id":record, "title":"Storage",
            "position":"Store records in one file.", "rationale":"One file is simple.",
            "status":status, "requirement_ids":parents, "supersedes":[], "inputs":[]
        })))
        .unwrap();
}

/// Accepts the pending submission of the current revision. Opens it first
/// when the record has none.
fn accept_current(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) {
    let state = store
        .record_decision_state(scope, kind, &id(record))
        .unwrap();
    let proposal = match state.pending {
        Some(pending) => pending.proposal_id,
        None => {
            store
                .submit_record_review(SubmitRecordReview {
                    scope_id: scope.clone(),
                    actor: "author".into(),
                    record_kind: kind,
                    record_id: id(record),
                    declared_by: None,
                    title: "Review the record".into(),
                    summary: "Review the stored text.".into(),
                    confidence: None,
                    source_ids: Vec::new(),
                    evidence_references: Vec::new(),
                    builds_on: Vec::new(),
                    expected_revision: None,
                    revises: None,
                })
                .unwrap()
                .proposal_id
        }
    };
    store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: DispositionActor {
                identity_type: IdentityType::Human,
                id: "reviewer".into(),
                name: None,
            },
            proposal_id: proposal,
            decision: DispositionDecision::Accepted,
            rationale: None,
            canonical_artifact: Some(CanonicalArtifact {
                artifact_type: input::<CanonicalArtifactType>(json!(kind.as_str())),
                artifact_id: id(record),
            }),
            feedback: None,
            declared_by: None,
        })
        .unwrap();
}

fn update_rule(store: &StateStore, change: &Value) -> anyhow::Result<provenance_core::Rule> {
    let mut request = json!({"scope_id":"default", "id":"rule_a"});
    request
        .as_object_mut()
        .unwrap()
        .extend(change.as_object().unwrap().clone());
    store.update_rule(input::<UpdateRuleInput>(request))
}

fn etag(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) -> String {
    store
        .record_edit_state(scope, kind, &id(record))
        .unwrap()
        .etag
}

fn status(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) -> Value {
    let value = match kind {
        NodeType::Requirement => serde_json::to_value(store.list_requirements(scope).unwrap()),
        NodeType::Rule => serde_json::to_value(store.list_rules(scope).unwrap()),
        _ => serde_json::to_value(store.list_resolutions(scope).unwrap()),
    }
    .unwrap();
    value
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == record)
        .unwrap()["status"]
        .clone()
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn creating_a_rule_or_resolution_opens_one_submission_and_a_status_move_opens_none() {
    let (_temp, store, scope) = fixture();
    requirement(&store, "req_a", &[]);
    rule(&store, "rule_a", "review", &["req_a"]);
    resolution(&store, "res_a", "review", &["req_a"]);
    rule(&store, "rule_b", "draft", &["req_a"]);

    for (kind, record) in [
        (NodeType::Rule, "rule_a"),
        (NodeType::Resolution, "res_a"),
        (NodeType::Rule, "rule_b"),
    ] {
        assert_eq!(submissions(&store, &scope, kind, record), 1, "{record}");
        assert_pending_at_current(&store, &scope, kind, record);
    }

    let etag_b = etag(&store, &scope, NodeType::Rule, "rule_b");
    store
        .update_rule(input(json!({
            "scope_id":"default", "id":"rule_b", "status":"review", "expected_etag":etag_b
        })))
        .unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_b"), 1);
    assert_pending_at_current(&store, &scope, NodeType::Rule, "rule_b");
}

fn typed_rule() -> TypedSpecInput {
    input(json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "spec":"storage", "declared_by":"spec://storage", "adopt_unowned":[],
        "sources":[{"key":"policy","id":"source_a","name":"Policy","kind":"document"}],
        "requirements":[{"key":"storage","id":"req_a","statement":"The system stores records.",
            "sources":["policy"]}],
        "rules":[{"key":"keep","id":"rule_a","requirements":["storage"],
            "statement":"The system keeps each record."}]
    }))
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn content_change_to_a_never_submitted_rule_opens_one_submission() {
    let (_temp, store, scope) = fixture();
    store.apply_typed_spec(&scope, typed_rule()).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 0);

    let owner = json!("spec://storage");
    update_rule(&store, &json!({"declared_by":owner, "status":"active"})).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 0);

    let guard = etag(&store, &scope, NodeType::Rule, "rule_a");
    update_rule(
        &store,
        &json!({"declared_by":owner, "expected_etag":guard,
               "statement":"The system keeps each stored record."}),
    )
    .unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 1);
    assert_pending_at_current(&store, &scope, NodeType::Rule, "rule_a");
    assert_eq!(status(&store, &scope, NodeType::Rule, "rule_a"), "active");

    update_rule(
        &store,
        &json!({"declared_by":owner, "statement":"The system keeps each stored record."}),
    )
    .unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 1);
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn archive_parent_pruning_opens_one_submission_for_each_surviving_child() {
    let (_temp, store, scope) = fixture();
    requirement(&store, "req_left", &[]);
    requirement(&store, "req_right", &[]);
    requirement(&store, "req_child", &["req_left", "req_right"]);
    rule(&store, "rule_child", "active", &["req_left", "req_right"]);
    resolution(&store, "res_child", "proposed", &["req_left", "req_right"]);
    rule(&store, "rule_only", "active", &["req_left"]);
    let children = [
        (NodeType::Requirement, "req_child"),
        (NodeType::Rule, "rule_child"),
        (NodeType::Resolution, "res_child"),
        (NodeType::Rule, "rule_only"),
    ];
    let mut before = Vec::new();
    for (kind, record) in children {
        accept_current(&store, &scope, kind, record);
        before.push((
            submissions(&store, &scope, kind, record),
            status(&store, &scope, kind, record),
        ));
    }

    store
        .update_requirement(input::<UpdateRequirementInput>(json!({
            "scope_id":"default", "id":"req_left", "status":"archived",
            "archived_in_commit":{"commit":COMMIT}
        })))
        .unwrap();

    for ((kind, record), (count, old_status)) in children.into_iter().take(3).zip(&before) {
        assert_eq!(
            submissions(&store, &scope, kind, record),
            count + 1,
            "{record}"
        );
        assert_pending_at_current(&store, &scope, kind, record);
        assert_eq!(
            &status(&store, &scope, kind, record),
            old_status,
            "{record}"
        );
    }
    // The cascade only archives this Rule: a lifecycle change opens nothing.
    assert_eq!(
        status(&store, &scope, NodeType::Rule, "rule_only"),
        "archived"
    );
    assert_eq!(
        submissions(&store, &scope, NodeType::Rule, "rule_only"),
        before[3].0
    );
    let state = store
        .record_decision_state(&scope, NodeType::Rule, &id("rule_only"))
        .unwrap();
    assert!(state.pending.is_none());
    assert!(state.current_acceptance.is_some());
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
#[verifies("rule_review_conflict_not_merged", examples)]
fn two_guarded_rule_writes_with_one_etag_publish_one_change_and_one_submission() {
    let (_temp, store, scope) = fixture();
    requirement(&store, "req_a", &[]);
    rule(&store, "rule_a", "active", &["req_a"]);
    let before = submissions(&store, &scope, NodeType::Rule, "rule_a");
    let guard = etag(&store, &scope, NodeType::Rule, "rule_a");
    let barrier = std::sync::Barrier::new(2);

    let results = std::thread::scope(|threads| {
        let writers = ["The system keeps record A.", "The system keeps record B."].map(|text| {
            let (store, guard, barrier) = (&store, &guard, &barrier);
            threads.spawn(move || {
                barrier.wait();
                update_rule(store, &json!({"expected_etag":guard, "statement":text}))
            })
        });
        writers.map(|writer| writer.join().unwrap())
    });

    let (winners, losers): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    let [Ok(winner)] = <[_; 1]>::try_from(winners).unwrap() else {
        panic!("one guarded write succeeds")
    };
    let [Err(loser)] = <[_; 1]>::try_from(losers).unwrap() else {
        panic!("one guarded write loses")
    };
    assert!(matches!(
        WriteError(loser).safe(),
        WriteFailure::RecordEditConflict { .. }
    ));
    let stored = store.list_rules(&scope).unwrap().remove(0);
    assert_eq!(stored.statement, winner.statement);
    assert_eq!(
        submissions(&store, &scope, NodeType::Rule, "rule_a"),
        before + 1
    );
    assert_pending_at_current(&store, &scope, NodeType::Rule, "rule_a");
}
