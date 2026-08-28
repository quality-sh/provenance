//! The named explicit import gate: on an rbac-managed repository the import
//! claim must hold `manifest-write` on every scope then listed (settled
//! Option A), so adding a scope narrows repo-global import authority.

use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;

fn provenance() -> Command {
    Command::cargo_bin("provenance").unwrap()
}

fn init_repo(directory: &std::path::Path) {
    provenance()
        .args([
            "init",
            "--path",
            directory.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn manifest_path(directory: &std::path::Path) -> std::path::PathBuf {
    directory.join(".provenance/state/manifest.json")
}

fn install_manifest(directory: &std::path::Path, body: &str) {
    std::fs::write(manifest_path(directory), body).unwrap();
}

fn grants_with_scopes(scopes: &str, extra_assignments: &str) -> String {
    format!(
        r#"{{
        "schema_version": 1,
        "scopes": [{scopes}],
        "disposition_actor_ids": [],
        "rbac": {extra_assignments}
    }}"#
    )
}

fn reviewer(capabilities: &str, scopes: &str) -> String {
    format!(
        r#"{{"assignments": [{{"actor_id": "reviewer", "identity_type": "human",
            "capabilities": [{capabilities}], "scopes": [{scopes}]}}]}}"#
    )
}

fn export_scope(directory: &std::path::Path) -> std::path::PathBuf {
    let output = directory.join("export.json");
    provenance()
        .args([
            "export",
            "--repo",
            directory.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    output
}

fn import_assert(
    directory: &std::path::Path,
    export: &std::path::Path,
) -> assert_cmd::assert::Assert {
    provenance()
        .args([
            "--actor-id",
            "reviewer",
            "import",
            "--repo",
            directory.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            export.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
}

#[test]
fn import_demands_manifest_write_on_every_scope_then_listed() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    provenance()
        .args([
            "--actor-id",
            "seeder",
            "sources",
            "create",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "source_policy",
            "--name",
            "Policy",
        ])
        .assert()
        .success();
    let export = export_scope(directory.path());
    install_manifest(
        directory.path(),
        &grants_with_scopes(
            r#"{"id": "default", "path_prefix": "."}"#,
            &reviewer("\"edit\"", "\"default\""),
        ),
    );

    // `edit` alone is not `manifest-write`: the swap is refused.
    import_assert(directory.path(), &export)
        .failure()
        .stderr(contains(
            "rbac: actor reviewer does not hold capability manifest-write on scope default",
        ));
}

#[test]
fn adding_a_scope_narrows_import_until_grants_cover_it() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    provenance()
        .args([
            "--actor-id",
            "seeder",
            "sources",
            "create",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "source_policy",
            "--name",
            "Policy",
        ])
        .assert()
        .success();
    let export = export_scope(directory.path());
    install_manifest(
        directory.path(),
        &grants_with_scopes(
            r#"{"id": "default", "path_prefix": "."}, {"id": "docs", "path_prefix": "docs"}"#,
            &reviewer("\"manifest-write\"", "\"default\""),
        ),
    );

    // Option A: the capability must be held on every scope then listed, so
    // the new `docs` scope blocks the repo-global swap.
    import_assert(directory.path(), &export)
        .failure()
        .stderr(contains(
            "rbac: actor reviewer does not hold capability manifest-write on scope docs",
        ));

    install_manifest(
        directory.path(),
        &grants_with_scopes(
            r#"{"id": "default", "path_prefix": "."}, {"id": "docs", "path_prefix": "docs"}"#,
            &reviewer("\"manifest-write\"", "\"default\", \"docs\""),
        ),
    );
    import_assert(directory.path(), &export).success();

    // A successful import preserves the section.
    let manifest = std::fs::read_to_string(manifest_path(directory.path())).unwrap();
    assert!(manifest.contains("\"rbac\""), "{manifest}");
}
