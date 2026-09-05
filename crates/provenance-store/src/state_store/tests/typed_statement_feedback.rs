use super::initialized_store;
use crate::state_store::{StateStore, TypedSpecInput};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

#[test]
#[verifies("rule_ste_typed_analysis_selection", examples)]
#[verifies("rule_ste_typed_diagnostic_parity", examples)]
fn plan_reports_created_requirements_and_rules_in_stable_order_with_utf8_spans() {
    let (_directory, store, scope) = initialized_store();

    let result = store
        .plan_typed_spec(
            &scope,
            spec("Café; requirement", "Rule; first; second", None),
        )
        .unwrap();
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(
        value["diagnostics"],
        json!([
            diagnostic(
                ["feedback", "requirement", "sharing"].as_slice(),
                "requirement",
                5
            ),
            diagnostic(
                ["feedback", "requirement", "sharing", "rule", "expiry"].as_slice(),
                "rule",
                4
            ),
            diagnostic(
                ["feedback", "requirement", "sharing", "rule", "expiry"].as_slice(),
                "rule",
                11
            )
        ])
    );
}

#[test]
#[verifies("rule_ste_typed_analysis_selection", examples)]
fn plan_reports_statement_changed_requirements_and_rules() {
    let (_directory, store, scope) = initialized_store();
    store
        .apply_typed_spec(&scope, spec("Clean requirement", "Clean rule", None))
        .unwrap();

    let result = store
        .plan_typed_spec(&scope, spec("Changed; requirement", "Changed; rule", None))
        .unwrap();
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(value["updated"], 2);
    assert_eq!(value["diagnostics"].as_array().unwrap().len(), 2);
    assert_eq!(value["diagnostics"][0]["resource_kind"], "requirement");
    assert_eq!(value["diagnostics"][1]["resource_kind"], "rule");
}

#[test]
#[verifies("rule_ste_typed_analysis_selection", examples)]
fn unrelated_update_allows_an_unchanged_grandfathered_invalid_statement() {
    let (_directory, store, scope) = initialized_store();
    store
        .apply_typed_spec(&scope, spec("Clean requirement", "Clean rule", None))
        .unwrap();
    grandfather_requirement(&store, "Grandfathered; requirement");

    let result = store
        .apply_typed_spec(
            &scope,
            spec(
                "Grandfathered; requirement",
                "Clean rule",
                Some("New description"),
            ),
        )
        .unwrap();
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(value["updated"], 1);
    assert!(value.get("diagnostics").is_none());
    assert_eq!(
        store.list_requirements(&scope).unwrap()[0]
            .description
            .as_deref(),
        Some("New description")
    );
}

#[test]
#[verifies("rule_ste_typed_diagnostic_parity", examples)]
#[verifies("rule_ste_typed_apply_atomic_rejection", examples)]
fn apply_returns_plan_diagnostics_before_any_state_edge_or_binding_write() {
    let (_directory, store, scope) = initialized_store();
    store
        .apply_typed_spec(&scope, spec("Clean requirement", "Clean rule", None))
        .unwrap();
    let rejected = spec_with_source_and_implementation("Changed; requirement");
    let planned =
        serde_json::to_value(store.plan_typed_spec(&scope, rejected.clone()).unwrap()).unwrap();
    let before = canonical_state(&store);

    let error = store.apply_typed_spec(&scope, rejected).unwrap_err();
    let typed = error
        .downcast_ref::<crate::state_store::TypedSpecWriteError>()
        .expect("apply preserves its typed STE100 report");
    assert_eq!(
        serde_json::to_value(&typed.diagnostics).unwrap(),
        planned["diagnostics"]
    );
    let report: Value = serde_json::from_str(&error.to_string()).unwrap();

    assert_eq!(report["error"], "asd_ste100_violations");
    assert_eq!(report["diagnostics"], planned["diagnostics"]);
    assert_eq!(canonical_state(&store), before);
}

fn spec(requirement: &str, rule: &str, description: Option<&str>) -> TypedSpecInput {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "feedback",
        "declared_by": "spec://typescript/feedback",
        "requirements": [{
            "key": "sharing",
            "statement": requirement,
            "description": description
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": rule
        }]
    }))
    .unwrap()
}

fn spec_with_source_and_implementation(requirement: &str) -> TypedSpecInput {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "feedback",
        "declared_by": "spec://typescript/feedback",
        "sources": [{
            "key": "policy",
            "name": "Policy",
            "kind": "policy"
        }],
        "requirements": [{
            "key": "sharing",
            "statement": requirement,
            "sources": ["policy"]
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Clean rule",
            "implementation": {"file": "src/feedback.ts", "symbol": "feedback"}
        }]
    }))
    .unwrap()
}

fn diagnostic(address: &[&str], resource_kind: &str, start: usize) -> Value {
    json!({
        "address": address,
        "resource_kind": resource_kind,
        "field": "statement",
        "standard": "ASD-STE100",
        "issue": 9,
        "rule": "8.1",
        "disposition": "violation",
        "span": {"start": start, "end": start + 1},
        "message": "Do not use semicolons in descriptive text."
    })
}

fn grandfather_requirement(store: &StateStore, statement: &str) {
    let path = store
        .layout
        .state_dir()
        .join("scopes/default/requirements/req.jsonl");
    let contents = std::fs::read_to_string(&path).unwrap();
    let mut requirement: Value = serde_json::from_str(contents.trim()).unwrap();
    requirement["statement"] = json!(statement);
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string(&requirement).unwrap()),
    )
    .unwrap();
}

fn canonical_state(store: &StateStore) -> BTreeMap<String, Vec<u8>> {
    let root = store.layout.state_dir();
    let mut state = BTreeMap::new();
    collect_files(root.as_std_path(), root.as_std_path(), &mut state);
    state
}

fn collect_files(root: &Path, current: &Path, state: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries = std::fs::read_dir(current)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            collect_files(root, &path, state);
        } else {
            state.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read(path).unwrap(),
            );
        }
    }
}
