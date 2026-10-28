use super::initialized_store;
use crate::{
    state_store::{TypedSpecInput, UpdateRuleInput, UpdateSourceInput},
    write_error::{WriteError, WriteFailure},
};
use provenance_core::{
    protocol::{TypedRequirementInput, TypedRuleInput, TypedSourceInput},
    NodeType, SUPPORTED_SCHEMA_VERSION,
};

fn document(source_name: &str, rule_statement: &str) -> TypedSpecInput {
    TypedSpecInput {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "review-history".into(),
        declared_by: "spec://review/history".into(),
        adopt_unowned: Vec::new(),
        sources: vec![TypedSourceInput {
            key: "policy".into(),
            id: Some("source_typed_history".into()),
            name: source_name.into(),
            kind: "document".into(),
            url: None,
            reference: None,
            supersedes: None,
        }],
        requirements: vec![TypedRequirementInput {
            key: "storage".into(),
            id: Some("requirement_typed_history".into()),
            statement: "The system stores records.".into(),
            description: None,
            sources: vec!["policy".into()],
            refines: None,
            depends_on: None,
            supersedes: None,
            spawned_by: None,
        }],
        rules: vec![TypedRuleInput {
            key: "retention".into(),
            id: Some("rule_typed_history".into()),
            address: None,
            requirement: None,
            requirements: vec!["storage".into()],
            statement: rule_statement.into(),
            name: None,
            description: None,
            implementation: None,
            resolution_ids: None,
        }],
    }
}

fn assert_deletion_conflict(error: anyhow::Error, kind: NodeType, id: &str) {
    let error = WriteError(error);
    assert!(matches!(
        error.safe(),
        WriteFailure::EnrolledRecordDeletionConflict {
            record_kind,
            record_id,
        } if record_kind == kind && record_id.as_str() == id
    ));
    assert_eq!(error.status(), 409);
}

#[test]
fn empty_typed_replacement_refuses_enrolled_source_deletion() {
    let (_temp, store, scope) = initialized_store();
    let mut input = document("Policy A", "The system retains A.");
    input.requirements.clear();
    input.rules.clear();
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    store
        .update_source(
            serde_json::from_value::<UpdateSourceInput>(serde_json::json!({
                "scope_id": "default",
                "id": "source_typed_history",
                "declared_by": "spec://review/history",
                "review_date": 1
            }))
            .unwrap(),
        )
        .unwrap();
    // A native edit does not itself submit this record for review.
    store
        .submit_record_review(
            serde_json::from_value(serde_json::json!({
                "scope_id":scope, "actor":"author", "record_kind":"source",
                "record_id":"source_typed_history", "declared_by":"spec://review/history",
                "title":"Review the record", "summary":"Review the stored text.",
                "source_ids":[], "evidence_references":[], "builds_on":[]
            }))
            .unwrap(),
        )
        .unwrap();
    input.sources.clear();

    let error = store.apply_typed_spec(&scope, input).unwrap_err();

    assert_deletion_conflict(error, NodeType::Source, "source_typed_history");
}

#[test]
fn empty_typed_rule_replacement_refuses_enrolled_rule_deletion() {
    let (_temp, store, scope) = initialized_store();
    let mut input = document("Policy A", "The system retains A.");
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    store
        .update_rule(
            serde_json::from_value::<UpdateRuleInput>(serde_json::json!({
                "scope_id": "default",
                "id": "rule_typed_history",
                "declared_by": "spec://review/history",
                "name": "Native rule name"
            }))
            .unwrap(),
        )
        .unwrap();
    // A native edit does not itself submit this record for review.
    store
        .submit_record_review(
            serde_json::from_value(serde_json::json!({
                "scope_id":scope, "actor":"author", "record_kind":"rule",
                "record_id":"rule_typed_history", "declared_by":"spec://review/history",
                "title":"Review the record", "summary":"Review the stored text.",
                "source_ids":[], "evidence_references":[], "builds_on":[]
            }))
            .unwrap(),
        )
        .unwrap();
    input.rules.clear();

    let error = store.apply_typed_spec(&scope, input).unwrap_err();

    assert_deletion_conflict(error, NodeType::Rule, "rule_typed_history");
}
