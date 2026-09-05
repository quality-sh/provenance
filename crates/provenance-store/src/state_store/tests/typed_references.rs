//! What a typed declaration does to the reference fields: a named field is
//! authoritative, an omitted field is untouched, and a bad target refuses.

use super::initialized_store;
use crate::state_store::{CreateResolutionInput, TypedSpecInput};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{ResolutionStatus, StableId};
use serde_json::json;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn spec(requirements: &serde_json::Value, rules: &serde_json::Value) -> TypedSpecInput {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "payroll",
        "declared_by": "spec://payroll",
        "sources": [{"key": "award", "name": "Award", "kind": "policy"}],
        "requirements": requirements,
        "rules": rules,
    }))
    .unwrap()
}

fn requirement(key: &str, extra: &serde_json::Value) -> serde_json::Value {
    let mut declaration =
        json!({"key": key, "statement": format!("{key} holds"), "sources": ["award"]});
    for (name, value) in extra.as_object().unwrap() {
        declaration[name] = value.clone();
    }
    declaration
}

#[test]
fn a_named_field_is_authoritative_and_an_omitted_field_stays() {
    let (_dir, store, scope) = initialized_store();
    let first = spec(
        &json!([
            requirement("pay", &json!({})),
            requirement(
                "overtime",
                &json!({"refines": "pay", "depends_on": ["pay"]})
            ),
        ]),
        &json!([]),
    );
    store.apply_typed_spec(&scope, first).unwrap();
    let overtime = |store: &crate::state_store::StateStore| {
        store
            .list_requirements(&scope)
            .unwrap()
            .into_iter()
            .find(|record| record.id.as_str().contains("overtime"))
            .unwrap()
    };
    let written = overtime(&store);
    assert!(written.refines.as_ref().unwrap().as_str().contains("pay"));
    assert_eq!(written.depends_on.len(), 1);

    let silent = spec(
        &json!([
            requirement("pay", &json!({})),
            requirement("overtime", &json!({}))
        ]),
        &json!([]),
    );
    store.apply_typed_spec(&scope, silent).unwrap();
    let kept = overtime(&store);
    assert_eq!(
        kept.refines, written.refines,
        "an omitted refines is untouched"
    );
    assert_eq!(
        kept.depends_on, written.depends_on,
        "an omitted list is untouched"
    );

    let cleared = spec(
        &json!([
            requirement("pay", &json!({})),
            requirement("overtime", &json!({"depends_on": []})),
        ]),
        &json!([]),
    );
    store.apply_typed_spec(&scope, cleared).unwrap();
    let emptied = overtime(&store);
    assert!(emptied.depends_on.is_empty(), "a present empty list clears");
    assert_eq!(emptied.refines, written.refines);
}

// One test per reference field a typed declaration can carry: a key the
// document does not declare is a typed refusal, never a lookup panic.
// `spawned_by` and `resolution_ids` name canonical resolutions and are
// refused by the resolution check; the rest name declaration keys and are
// refused by the document check.
fn spec_with_sources(
    sources: &serde_json::Value,
    requirements: &serde_json::Value,
    rules: &serde_json::Value,
) -> TypedSpecInput {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "payroll",
        "declared_by": "spec://payroll",
        "sources": sources,
        "requirements": requirements,
        "rules": rules,
    }))
    .unwrap()
}

#[test]
fn an_undeclared_citation_key_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({"sources": ["ghost"]}))]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "requirement `pay` references undeclared source `ghost`"
    );
}

#[test]
fn an_undeclared_refines_key_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({"refines": "ghost"}))]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "requirement `pay` references undeclared requirement `ghost`"
    );
}

#[test]
fn an_undeclared_depends_on_key_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({"depends_on": ["ghost"]}))]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "requirement `pay` references undeclared requirement `ghost`"
    );
}

#[test]
fn an_undeclared_supersedes_key_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({"supersedes": ["ghost"]}))]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "requirement `pay` references undeclared requirement `ghost`"
    );
}

#[test]
fn an_undeclared_spawned_by_resolution_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({"spawned_by": "res_ghost"}))]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(error, "resolution res_ghost does not exist (spawned_by)");
}

#[test]
fn a_source_superseding_an_undeclared_key_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec_with_sources(
        &json!([{"key": "award", "name": "Award", "kind": "policy", "supersedes": ["ghost"]}]),
        &json!([requirement("pay", &json!({}))]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(error, "source `award` references undeclared source `ghost`");
}

#[test]
fn a_rule_naming_an_undeclared_requirement_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({}))]),
        &json!([{"key": "expiry", "requirements": ["ghost"], "statement": "Pay expires"}]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "rule `expiry` references undeclared requirement `ghost`"
    );
}

#[test]
fn a_rule_naming_an_undeclared_resolution_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let input = spec(
        &json!([requirement("pay", &json!({}))]),
        &json!([{"key": "expiry", "requirements": ["pay"], "statement": "Pay expires",
            "resolution_ids": ["res_ghost"]}]),
    );
    let error = store
        .apply_typed_spec(&scope, input)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "resolution res_ghost does not exist (resolution_ids)"
    );
}

#[test]
fn a_refines_cycle_and_a_missing_resolution_are_refused() {
    let (_dir, store, scope) = initialized_store();
    let cycle = spec(
        &json!([
            requirement("pay", &json!({"refines": "overtime"})),
            requirement("overtime", &json!({"refines": "pay"})),
        ]),
        &json!([]),
    );
    let error = store
        .apply_typed_spec(&scope, cycle)
        .unwrap_err()
        .to_string();
    assert!(error.contains("refines from"), "{error}");
    assert!(error.contains("returns to itself"), "{error}");

    let missing = spec(
        &json!([requirement("pay", &json!({}))]),
        &json!([{"key": "expiry", "requirements": ["pay"], "statement": "Pay expires",
            "resolution_ids": ["res_missing"]}]),
    );
    let error = store
        .apply_typed_spec(&scope, missing)
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "resolution res_missing does not exist (resolution_ids)"
    );
}

#[test]
fn a_rule_carries_its_requirements_and_a_named_resolution() {
    let (_dir, store, scope) = initialized_store();
    store
        .apply_typed_spec(
            &scope,
            spec(&json!([requirement("pay", &json!({}))]), &json!([])),
        )
        .unwrap();
    let requirement_id = store.list_requirements(&scope).unwrap()[0].id.clone();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: sid("res_pay"),
            title: "Pay".into(),
            requirement_ids: vec![requirement_id.clone()],
            supersedes: Vec::new(),
            position: "Pay it".into(),
            rationale: "The award".into(),
            status: ResolutionStatus::Approved,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let produced = spec(
        &json!([requirement("pay", &json!({}))]),
        &json!([{"key": "expiry", "requirements": ["pay"], "statement": "Pay expires",
            "resolution_ids": ["res_pay"]}]),
    );
    store.apply_typed_spec(&scope, produced).unwrap();
    let rule = &store.list_rules(&scope).unwrap()[0];
    assert_eq!(rule.requirement_ids, vec![requirement_id]);
    assert_eq!(rule.resolution_ids, vec![sid("res_pay")]);
}
