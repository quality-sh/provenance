//! `schema show --artifact manifest` and `validate manifest` surface the
//! closed shape of the manifest, including the rbac section.

use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;

fn schema_show_manifest() -> serde_json::Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["schema", "show", "manifest", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "schema show manifest failed");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn schema_show_manifest_names_the_rbac_section_and_all_four_capabilities() {
    let report = schema_show_manifest();
    let rendered = serde_json::to_string(&report).unwrap();
    for word in [
        "rbac",
        "assignments",
        "actor_id",
        "identity_type",
        "read",
        "edit",
        "execute",
        "manifest-write",
    ] {
        assert!(
            rendered.contains(word),
            "schema must name {word}: {rendered}"
        );
    }
}

#[test]
fn validate_manifest_accepts_a_closed_manifest_and_refuses_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good-manifest.json");
    std::fs::write(
        &good,
        r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": [],
        "rbac": {"assignments": [{
            "actor_id": "reviewer",
            "identity_type": "human",
            "capabilities": ["edit"],
            "scopes": ["default"]
        }]}
    }"#,
    )
    .unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["validate", "manifest", "--input"])
        .arg(&good)
        .assert()
        .success();

    let bad = dir.path().join("bad-manifest.json");
    std::fs::write(
        &bad,
        r#"{
        "schema_version": 1,
        "scopes": [],
        "disposition_actor_ids": [],
        "rbac": {"assignments": [], "wildcard": true}
    }"#,
    )
    .unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["validate", "manifest", "--input"])
        .arg(&bad)
        .assert()
        .failure()
        .stderr(contains("unknown"));
}
