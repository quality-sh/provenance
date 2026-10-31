//! The review revision classifier over records held in memory, with no store.

use provenance_core::{NodeType, Requirement, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::verifies;
use provenance_store::review::{changed_fields, changes_revision, content_digest, review_revision};
use serde_json::json;

fn requirement(statement: &str, status: &str) -> Requirement {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "req_overtime",
        "statement": statement,
        "status": status,
    }))
    .unwrap()
}

#[test]
#[verifies("rule_review_revision_follows_review_content", examples)]
fn a_status_change_keeps_the_revision_and_a_statement_change_does_not() {
    let active = requirement("Overtime is paid", "active");
    let discovery = requirement("Overtime is paid", "discovery");
    let restated = requirement("Overtime is paid at a higher rate", "active");

    let fields = changed_fields(NodeType::Requirement, &active, &discovery).unwrap();

    assert_eq!(fields, ["status"]);
    assert!(!changes_revision(NodeType::Requirement, &fields));
    assert_eq!(
        review_revision(NodeType::Requirement, &active).unwrap(),
        review_revision(NodeType::Requirement, &discovery).unwrap()
    );
    assert_eq!(
        content_digest(NodeType::Requirement, &active).unwrap(),
        content_digest(NodeType::Requirement, &discovery).unwrap()
    );

    let fields = changed_fields(NodeType::Requirement, &active, &restated).unwrap();

    assert_eq!(fields, ["statement"]);
    assert!(changes_revision(NodeType::Requirement, &fields));
    assert_ne!(
        review_revision(NodeType::Requirement, &active).unwrap(),
        review_revision(NodeType::Requirement, &restated).unwrap()
    );
}
