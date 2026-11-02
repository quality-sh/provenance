//! A record write and its automatic review submission publish together, or
//! neither publishes.

use crate::{layout::ProvenanceLayout, shards, state_store::StateStore, test_probes};
use camino::Utf8Path;
use provenance_core::ScopeId;
use provenance_macros::verifies;
use serde_json::json;

const PROBE: &str = "automatic_submission_before_commit";

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = crate::test_support::store(&layout);
    let scope = ScopeId::new("default").unwrap();
    store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"req_a", "statement":"The system stores records.",
                "status":"active", "depends_on":[], "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
    (temp, store, scope)
}

/// The bytes of the Rule and proposal sections, absent ones as `None`.
fn sections(store: &StateStore, scope: &ScopeId) -> [Option<Vec<u8>>; 2] {
    [
        shards::rules_path(&store.layout, scope),
        shards::proposal_cards_path(&store.layout, scope),
    ]
    .map(|path| std::fs::read(path).ok())
}

fn create_rule(store: &StateStore) -> anyhow::Result<provenance_core::Rule> {
    store.create_rule(serde_json::from_value(json!({
        "scope_id":"default", "id":"rule_a", "statement":"The system keeps each record.",
        "status":"review", "severity":"medium", "requirement_ids":["req_a"],
        "resolution_ids":[]
    }))?)
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn failed_submission_publishes_neither_the_created_record_nor_the_submission() {
    let (_temp, store, scope) = fixture();
    let before = sections(&store, &scope);

    test_probes::crash_at(PROBE);
    let result = create_rule(&store);
    test_probes::disarm(PROBE);

    assert!(
        result.is_err(),
        "the create published without its submission"
    );
    assert_eq!(sections(&store, &scope), before);
    assert!(store.list_rules(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn failed_submission_publishes_neither_the_content_change_nor_the_submission() {
    let (_temp, store, scope) = fixture();
    create_rule(&store).unwrap();
    let before = sections(&store, &scope);

    test_probes::crash_at(PROBE);
    let result = store.update_rule(
        serde_json::from_value(json!({
            "scope_id":"default", "id":"rule_a",
            "statement":"The system keeps each stored record."
        }))
        .unwrap(),
    );
    test_probes::disarm(PROBE);

    assert!(
        result.is_err(),
        "the change published without its submission"
    );
    assert_eq!(sections(&store, &scope), before);
}
