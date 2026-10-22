//! Characterization tests, not bound to a Rule: they pin which shard error
//! wins when a scope has a malformed Withdrawals shard and a malformed
//! Dispositions shard. The review state reads the Withdrawals first.
use super::*;

/// Writes one line of an unsupported schema version to each shard.
fn corrupt_withdrawals_and_dispositions(store: &StateStore) {
    let ideation = store.layout.state_dir().join("scopes/default/ideation");
    std::fs::create_dir_all(&ideation).unwrap();
    for (shard, id) in [
        ("withdrawals.jsonl", "withdrawal_bad"),
        ("dispositions.jsonl", "disposition_bad"),
    ] {
        std::fs::write(
            ideation.join(shard),
            format!("{}\n", json!({"schema_version":999,"id":id})),
        )
        .unwrap();
    }
}

fn reports_the_withdrawals_error<T: std::fmt::Debug>(attempt: anyhow::Result<T>) {
    let message = format!("{:#}", attempt.unwrap_err());
    assert!(message.contains("withdrawal_bad"), "{message}");
    assert!(!message.contains("disposition_bad"), "{message}");
}

#[test]
fn decision_state_read_reports_the_withdrawals_error_first() {
    let (_temp, store, _, _) = enrolled();
    corrupt_withdrawals_and_dispositions(&store);

    reports_the_withdrawals_error(store.requirement_decision_state(&scope(), &req()));
}

#[test]
fn revision_edit_reports_the_withdrawals_error_first_and_changes_nothing() {
    let (_temp, store, _, _) = enrolled();
    let before = store.requirement_edit_state(&scope(), &req()).unwrap().etag;
    corrupt_withdrawals_and_dispositions(&store);

    reports_the_withdrawals_error(store.save_requirement(
        serde_json::from_value(json!({
            "actor":"agent","expected_etag":before,
            "update":{"scope_id":"default","id":"req_a","statement":"Statement v2"},
            "relationships":null
        }))
        .unwrap(),
    ));

    let after = store.requirement_edit_state(&scope(), &req()).unwrap().etag;
    assert_eq!(before, after);
    let requirement = store.list_requirements(&scope()).unwrap().remove(0);
    assert_eq!(requirement.statement, "Statement v1");
}
