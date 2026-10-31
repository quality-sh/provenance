//! The typed-spec decision over a supplied record view.

use super::initialized_store;
use crate::state_store::{
    decide_typed_spec, CreateRequirementInput, ReconcileState, TypedResourceKind, TypedSpecInput,
    TypedSpecView,
};
use crate::write_error::{WriteError, WriteFailure};
use provenance_core::{
    Requirement, RequirementStatus, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;
use serde_json::json;

const OWNER: &str = "spec://payroll";

fn spec(requirements: &serde_json::Value) -> TypedSpecInput {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "payroll",
        "declared_by": OWNER,
        "sources": [{"key": "award", "name": "Award", "kind": "policy"}],
        "requirements": requirements,
        "rules": [],
    }))
    .unwrap()
}

fn requirement(key: &str) -> serde_json::Value {
    json!({"key": key, "statement": format!("{key} holds"), "sources": ["award"]})
}

#[test]
fn an_omitted_owned_requirement_is_deleted_from_records_in_memory() {
    let scope = ScopeId::new("default").unwrap();
    let owned: Requirement = serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "req_payroll_pay",
        "declared_by": OWNER,
        "declaration_address": ["payroll", "requirement", "pay"],
        "statement": "pay holds",
        "status": "active",
    }))
    .unwrap();
    let view = TypedSpecView {
        requirements: vec![owned],
        ..TypedSpecView::default()
    };

    let decision = decide_typed_spec(&scope, spec(&json!([])), &view).unwrap();

    let deleted = decision
        .result
        .resources
        .iter()
        .filter(|resource| resource.state == ReconcileState::Deleted)
        .collect::<Vec<_>>();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].kind, TypedResourceKind::Requirement);
    assert_eq!(deleted[0].id.as_str(), "req_payroll_pay");
    assert_eq!(decision.result.deleted, 1);
}

#[test]
#[verifies("rule_typed_spec_decision_reads_only_supplied_records", examples)]
fn the_plan_equals_the_decision_over_the_view_the_store_reads() {
    let (_dir, store, scope) = initialized_store();
    store
        .apply_typed_spec(
            &scope,
            spec(&json!([requirement("pay"), requirement("overtime")])),
        )
        .unwrap();
    let next = spec(&json!([requirement("pay"), requirement("leave")]));

    let plan = store.plan_typed_spec(&scope, next.clone()).unwrap();
    let view = store.typed_spec_view(&scope).unwrap();
    let decision = decide_typed_spec(&scope, next, &view).unwrap();

    assert_eq!(decision.result, plan);
    assert_eq!((plan.created, plan.deleted), (1, 1));
}

#[test]
#[verifies(
    "rule_typed_spec_apply_decides_inside_its_publication_section",
    examples
)]
fn an_apply_refuses_a_plan_that_a_native_write_made_stale() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(&json!([requirement("pay")]));
    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    let planned = plan
        .resources
        .iter()
        .find(|resource| resource.kind == TypedResourceKind::Requirement)
        .unwrap();
    assert_eq!(planned.state, ReconcileState::Created);
    let native = store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new(planned.id.as_str()).unwrap(),
            statement: "A person wrote this".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: Vec::new(),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    let error = WriteError(store.apply_typed_spec(&scope, input).unwrap_err());

    let WriteFailure::OwnershipConflict { conflicts } = error.safe() else {
        panic!("expected an ownership conflict, got {error}")
    };
    assert_eq!(conflicts[0].id, planned.id);
    assert_eq!(store.list_requirements(&scope).unwrap(), vec![native]);
}
