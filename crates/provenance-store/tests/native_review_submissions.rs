//! Native record writes open their review submission in the same write: on
//! creation, on a content change to a record that was never submitted, on
//! archive parent pruning, and once for a race of two guarded writes.

#[path = "native_review_submissions/entry_paths.rs"]
mod entry_paths;
#[path = "native_review_submissions/support.rs"]
mod support;

use provenance_core::NodeType;
use provenance_macros::verifies;
use provenance_store::{
    state_store::UpdateRequirementInput,
    write_error::{WriteError, WriteFailure},
};
use serde_json::json;
use support::*;

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
#[test]
#[verifies("rule_native_create_opens_review_submission", examples)]
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
