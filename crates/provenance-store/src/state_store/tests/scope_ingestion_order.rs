use super::initialized_store;
use crate::state_store::ScopeShards;
use provenance_core::{Thread, VerificationBinding};

fn thread(id: &str, text: &str) -> Thread {
    serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": id,
        "parent": { "node_type": "requirement", "node_id": "req_anchor" },
        "status": "resolved",
        "opened_by": text,
        "opened_at": 0
    }))
    .unwrap()
}

fn verification_binding(id: &str, file: &str) -> VerificationBinding {
    serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": id,
        "rule_id": "rule_anchor",
        "key": "check",
        "method": "examples",
        "declared_by": "ci://test",
        "file": file
    }))
    .unwrap()
}

#[test]
fn assignability_checks_verification_bindings_before_canonical_families() {
    let (_dir, store, scope) = initialized_store();
    let threads = [thread("search", "worker")];
    let bindings = [verification_binding("check", "tests/check.rs")];

    let error = store
        .import_scope(
            &scope,
            &ScopeShards {
                threads: &threads,
                verification_bindings: &bindings,
                ..ScopeShards::default()
            },
        )
        .unwrap_err();

    assert!(error.to_string().contains("reserved record ID check"), "{error}");
}

#[test]
fn budget_checks_verification_bindings_before_canonical_families() {
    let oversized = "x".repeat(crate::cache::read::page::RESOURCE_RECORD_BYTES);
    let threads = [thread("thread_large", &oversized)];
    let bindings = [verification_binding("verification_large", &oversized)];

    let (_expected_dir, expected_store, expected_scope) = initialized_store();
    let expected = expected_store
        .import_scope(
            &expected_scope,
            &ScopeShards {
                verification_bindings: &bindings,
                ..ScopeShards::default()
            },
        )
        .unwrap_err();

    let (_actual_dir, actual_store, actual_scope) = initialized_store();
    let actual = actual_store
        .import_scope(
            &actual_scope,
            &ScopeShards {
                threads: &threads,
                verification_bindings: &bindings,
                ..ScopeShards::default()
            },
        )
        .unwrap_err();

    assert_eq!(actual.to_string(), expected.to_string());
}
