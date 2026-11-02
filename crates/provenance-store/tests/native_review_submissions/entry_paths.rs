//! Typed-spec apply and the reference writers open one submission for each
//! content change, and none for identical content or content that a reviewer
//! accepted. A refused submission step publishes neither the record nor a
//! submission.

use super::support::*;
use provenance_core::NodeType;
use provenance_macros::verifies;
use provenance_store::{layout::ProvenanceLayout, shards};
use serde_json::json;

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn typed_rule_changes_open_one_submission_for_each_new_content() {
    let (_temp, store, scope) = fixture();
    let mut spec = typed_rule();
    store.apply_typed_spec(&scope, spec.clone()).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 0);

    spec.rules[0].statement = "The system keeps each stored record.".into();
    store.apply_typed_spec(&scope, spec.clone()).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 1);
    assert_pending_at_current(&store, &scope, NodeType::Rule, "rule_a");
    let state = store
        .record_decision_state(&scope, NodeType::Rule, &id("rule_a"))
        .unwrap();
    assert_eq!(state.pending.unwrap().actor, "spec://storage");

    // The same content again adds nothing.
    store.apply_typed_spec(&scope, spec.clone()).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 1);

    accept_current(&store, &scope, NodeType::Rule, "rule_a");
    spec.rules[0].statement = "The system keeps each record for one year.".into();
    store.apply_typed_spec(&scope, spec.clone()).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 2);
    assert_pending_at_current(&store, &scope, NodeType::Rule, "rule_a");

    // The accepted content returns: no submission, and the acceptance holds.
    spec.rules[0].statement = "The system keeps each stored record.".into();
    store.apply_typed_spec(&scope, spec).unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 2);
    let state = store
        .record_decision_state(&scope, NodeType::Rule, &id("rule_a"))
        .unwrap();
    assert!(state.pending.is_none());
    assert!(state.current_acceptance.is_some());
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn rule_reference_writes_open_one_submission_for_each_new_content() {
    let (_temp, store, scope) = fixture();
    requirement(&store, "req_a", &[]);
    requirement(&store, "req_b", &[]);
    rule(&store, "rule_a", "active", &["req_a"]);
    accept_current(&store, &scope, NodeType::Rule, "rule_a");
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 1);

    store
        .add_rule_requirement(&scope, &id("rule_a"), id("req_b"))
        .unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 2);
    assert_pending_at_current(&store, &scope, NodeType::Rule, "rule_a");
    assert_eq!(status(&store, &scope, NodeType::Rule, "rule_a"), "active");

    // The same link again does not change the content.
    store
        .add_rule_requirement(&scope, &id("rule_a"), id("req_b"))
        .unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 2);

    // The accepted content returns: no submission, and the acceptance holds.
    store
        .clear_rule_requirement(&scope, &id("rule_a"), &id("req_b"))
        .unwrap();
    assert_eq!(submissions(&store, &scope, NodeType::Rule, "rule_a"), 2);
    let state = store
        .record_decision_state(&scope, NodeType::Rule, &id("rule_a"))
        .unwrap();
    assert!(state.pending.is_none());
    assert!(state.current_acceptance.is_some());
}

#[test]
#[verifies("rule_native_create_opens_review_submission", examples)]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn a_refused_submission_publishes_neither_the_record_nor_a_submission() {
    let (temp, store, scope) = fixture();
    requirement(&store, "req_a", &[]);
    rule(&store, "rule_a", "active", &["req_a"]);
    // A Withdrawal that names no review submission makes the submission step
    // refuse: the review history is not valid.
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let withdrawals = shards::withdrawals_path(&layout, &scope);
    std::fs::create_dir_all(withdrawals.parent().unwrap()).unwrap();
    let invalid = json!({"scope_id":"default", "id":"withdrawal_a",
        "proposal_id":"proposal_missing", "actor":"agent", "reason":null});
    std::fs::write(&withdrawals, format!("{invalid}\n")).unwrap();
    let sections = || {
        [
            shards::rules_path(&layout, &scope),
            shards::proposal_cards_path(&layout, &scope),
        ]
        .map(|path| std::fs::read(path).unwrap())
    };
    let before = sections();

    let created = store.create_rule(input(json!({
        "scope_id":"default", "id":"rule_b", "statement":"The system keeps each record.",
        "status":"draft", "severity":"medium", "requirement_ids":["req_a"],
        "resolution_ids":[]
    })));
    let changed = update_rule(
        &store,
        &json!({"statement":"The system keeps each stored record."}),
    );

    for result in [created, changed] {
        let error = format!("{:#}", result.unwrap_err());
        assert!(error.contains("names no review submission"), "{error}");
    }
    assert_eq!(sections(), before);
}
