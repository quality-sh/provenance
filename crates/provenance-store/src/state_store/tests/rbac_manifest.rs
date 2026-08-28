//! The manifest reader laws for the `rbac` section: every reader refuses the
//! ambiguous both-regimes manifest and reads rbac-bearing manifests fine.

use super::initialized_store;
use provenance_core::{
    Manifest, RepoPathPrefix, SchemaVersion, ScopeId, AMBIGUOUS_MANIFEST_REFUSAL,
};

fn write_manifest(store: &super::super::StateStore, body: String) {
    std::fs::write(store.layout.manifest_path(), body).unwrap();
}

fn rbac_only_manifest_body() -> String {
    r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": [],
        "rbac": {"assignments": [{
            "actor_id": "reviewer",
            "identity_type": "human",
            "capabilities": ["read", "edit", "execute", "manifest-write"],
            "scopes": ["default"]
        }]}
    }"#
    .to_string()
}

fn ambiguous_manifest_body() -> String {
    r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": ["ben"],
        "rbac": {"assignments": [{
            "actor_id": "reviewer",
            "capabilities": ["edit"],
            "scopes": ["default"]
        }]}
    }"#
    .to_string()
}

#[test]
fn the_store_reader_refuses_the_ambiguous_manifest_with_the_fixed_golden() {
    let (_dir, store, _scope) = initialized_store();
    write_manifest(&store, ambiguous_manifest_body());

    let error = store.manifest().unwrap_err();
    assert_eq!(error.to_string(), AMBIGUOUS_MANIFEST_REFUSAL);
}

#[test]
fn the_store_reader_accepts_rbac_only_and_legacy_only_manifests() {
    let (_dir, store, _scope) = initialized_store();
    write_manifest(&store, rbac_only_manifest_body());
    let manifest = store.manifest().unwrap();
    assert_eq!(manifest.rbac.unwrap().assignments.len(), 1);

    write_manifest(
        &store,
        serde_json::to_string(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    );
    store.manifest().unwrap();
}

#[test]
fn the_closed_projection_reads_an_rbac_bearing_manifest() {
    let (_dir, store, scope) = initialized_store();
    write_manifest(&store, rbac_only_manifest_body());

    let (version, selected) = store.closed_manifest_scope(&scope).unwrap();
    assert_eq!(version, SchemaVersion(1));
    assert_eq!(selected.unwrap().id, scope);
}

#[test]
fn the_closed_projection_refuses_the_ambiguous_manifest() {
    let (_dir, store, scope) = initialized_store();
    write_manifest(&store, ambiguous_manifest_body());

    let outcome = store.closed_manifest_scope(&scope);
    let error = outcome.expect_err("both regimes must refuse through the closed projection");
    assert_eq!(error.to_string(), AMBIGUOUS_MANIFEST_REFUSAL);
}

fn malformed_section_manifest_body() -> String {
    r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": [],
        "rbac": {"assignments": [
            {"actor_id": "reviewer", "capabilities": ["edit"], "scopes": ["default"]},
            {"actor_id": "reviewer", "capabilities": ["read"], "scopes": ["default"]}
        ]}
    }"#
    .to_string()
}

#[test]
fn the_store_reader_refuses_a_malformed_section() {
    let (_dir, store, _scope) = initialized_store();
    write_manifest(&store, malformed_section_manifest_body());

    let error = store.manifest().unwrap_err();
    assert!(
        error.to_string().contains("duplicate rbac grant"),
        "a repeated (actor, scope) grant refuses through the store reader: {error}"
    );
}

#[test]
fn the_closed_projection_refuses_a_malformed_section() {
    let (_dir, store, scope) = initialized_store();
    write_manifest(&store, malformed_section_manifest_body());

    let outcome = store.closed_manifest_scope(&scope);
    let error = outcome.expect_err("a malformed section must refuse through the closed projection");
    assert!(
        error.to_string().contains("duplicate rbac grant"),
        "the closed projection runs the section well-formedness law: {error}"
    );
}
