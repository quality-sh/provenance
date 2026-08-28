//! The init-family laws: re-init authorization on every scope, the
//! Plan-mandated rbac byte preservation, and the window deprecation warning.

use crate::support::{
    grants, init_repo, install_manifest, manifest_path, provenance, reviewer_human, MISSING_CLAIM,
};
use assert_cmd::prelude::*;
use predicates::str::contains;

#[test]
fn reinit_of_an_rbac_repository_demands_manifest_write_on_every_scope() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(
        directory.path(),
        &grants(
            r#"{"assignments": [
                {"actor_id": "operator", "identity_type": "human", "capabilities": ["read", "edit", "execute", "manifest-write"], "scopes": ["default"]},
                {"actor_id": "reader", "identity_type": "human", "capabilities": ["read"], "scopes": ["default"]}
            ]}"#,
        ),
    );

    provenance()
        .args(["init", "--path", directory.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(MISSING_CLAIM));

    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--actor-id",
            "reader",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "rbac: actor reader does not hold capability manifest-write on scope default",
        ));

    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--actor-id",
            "operator",
        ])
        .assert()
        .success();
}

#[test]
fn reinit_with_flags_omitted_preserves_the_rbac_section() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    let section = grants(&reviewer_human(
        "\"read\",\"edit\",\"execute\",\"manifest-write\"",
    ));
    install_manifest(directory.path(), &section);
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--actor-id",
            "reviewer",
        ])
        .assert()
        .success();

    let after = std::fs::read_to_string(manifest_path(directory.path())).unwrap();
    let before: serde_json::Value = serde_json::from_str(&section).unwrap();
    let after: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(
        after, before,
        "the rbac section must survive re-init unchanged"
    );
}

#[test]
fn init_disposition_actor_flags_print_the_window_deprecation_warning() {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success()
        .stderr(contains(
            "warning: init --disposition-actor-id / --clear-disposition-actors are deprecated",
        ))
        .stderr(contains("rbac.assignments"));

    let fresh = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            fresh.path().to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success()
        .stderr(predicates::str::is_empty());
}
