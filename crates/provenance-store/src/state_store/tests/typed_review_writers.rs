use super::initialized_store;
use crate::{
    state_store::{TypedSpecInput, UpdateRuleInput, UpdateSourceInput},
    write_error::{WriteError, WriteFailure},
};
use provenance_core::{
    protocol::{TypedRequirementInput, TypedRuleInput, TypedSourceInput},
    review::SaveOutcome,
    NodeType, SchemaVersion, SUPPORTED_SCHEMA_VERSION,
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

fn entries_for(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    kind: NodeType,
) -> Vec<provenance_core::review::ReviewEntry> {
    let mut entries = store
        .review_entries(scope)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.record_kind == kind)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.sequence);
    entries
}

#[test]
fn typed_source_and_rule_record_creation_and_each_repeated_value_change() {
    let (_temp, store, scope) = initialized_store();
    store
        .apply_typed_spec(&scope, document("Policy A", "The system retains A."))
        .unwrap();
    store
        .apply_typed_spec(&scope, document("Policy B", "The system retains B."))
        .unwrap();
    store
        .apply_typed_spec(&scope, document("Policy A", "The system retains A."))
        .unwrap();

    assert_eq!(
        store.list_sources(&scope).unwrap()[0].schema_version,
        SchemaVersion(2)
    );
    assert_eq!(
        store.list_rules(&scope).unwrap()[0].schema_version,
        SchemaVersion(2)
    );
    for kind in [NodeType::Source, NodeType::Rule] {
        let entries = entries_for(&store, &scope, kind);
        assert_eq!(entries.len(), 3, "{kind:?}");
        assert_eq!(entries[0].outcome, SaveOutcome::Created, "{kind:?}");
        assert_eq!(entries[1].outcome, SaveOutcome::Changed, "{kind:?}");
        assert_eq!(entries[2].outcome, SaveOutcome::Changed, "{kind:?}");
        assert_eq!(entries[1].predecessor.as_ref(), Some(&entries[0].id));
        assert_eq!(entries[2].predecessor.as_ref(), Some(&entries[1].id));
    }
}

#[test]
fn typed_source_and_rule_recreation_links_a_second_creation_occurrence() {
    let (_temp, store, scope) = initialized_store();
    let input = document("Policy A", "The system retains A.");
    store.apply_typed_spec(&scope, input.clone()).unwrap();

    let mut empty = input.clone();
    empty.sources.clear();
    empty.requirements.clear();
    empty.rules.clear();
    store.apply_typed_spec(&scope, empty).unwrap();
    store.apply_typed_spec(&scope, input).unwrap();

    for kind in [NodeType::Source, NodeType::Rule] {
        let entries = entries_for(&store, &scope, kind);
        assert_eq!(entries.len(), 2, "{kind:?}");
        assert_eq!(entries[0].outcome, SaveOutcome::Created, "{kind:?}");
        assert_eq!(entries[1].outcome, SaveOutcome::Created, "{kind:?}");
        assert_eq!(entries[1].predecessor.as_ref(), Some(&entries[0].id));
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
    input.rules.clear();

    let error = store.apply_typed_spec(&scope, input).unwrap_err();

    assert_deletion_conflict(error, NodeType::Rule, "rule_typed_history");
}
