use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use std::path::Path;

fn command(root: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.current_dir(root).args(args);
    command.assert()
}

fn write_manifest(root: &Path, version: u32) {
    let state = root.join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("manifest.json"),
        serde_json::json!({
            "schema_version": version,
            "scopes": [{"id": "default", "path_prefix": "."}]
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn interrupted_publication_is_recovered_by_health() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_manifest(root, SUPPORTED_SCHEMA_VERSION.0);
    let state = root.join(".provenance/state");
    let cache = root.join(".provenance/cache");
    let transaction = cache.join("import-transactions/interrupted");
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::rename(&state, transaction.join("backup-state")).unwrap();
    std::fs::write(
        cache.join("import-publication.json"),
        serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "backup_created"
        })
        .to_string(),
    )
    .unwrap();

    command(root, &["health"]).success();
    assert!(state.join("manifest.json").is_file());
    assert!(!cache.join("import-publication.json").exists());
}

#[test]
fn health_rejects_invalid_manifests() {
    let unsupported = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0 + 2,
        "scopes": []
    })
    .to_string();
    for (content, expected) in [
        ("{", "EOF"),
        (unsupported.as_str(), "manifest schema_version must be"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let state = directory.path().join(".provenance/state");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("manifest.json"), content).unwrap();
        command(directory.path(), &["health"])
            .failure()
            .stderr(contains(expected));
    }
}

#[test]
fn a_manifest_directory_is_invalid_state() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join(".provenance/state/manifest.json")).unwrap();
    command(directory.path(), &["health"])
        .failure()
        .stderr(contains("manifest is not a file").and(contains("provenance init").not()));
}

#[cfg(unix)]
#[test]
fn a_broken_manifest_link_is_invalid_state() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    std::os::unix::fs::symlink("missing.json", state.join("manifest.json")).unwrap();
    command(directory.path(), &["health"])
        .failure()
        .stderr(contains("manifest is not a file").and(contains("provenance init").not()));
}

#[test]
fn an_empty_graph_check_keeps_the_repository_read_only() {
    for selector in ["--graph", "--statements", "--bindings"] {
        let directory = tempfile::tempdir().unwrap();
        command(directory.path(), &["check", selector, "--format", "json"])
            .failure()
            .stdout(contains(&selector[2..]))
            .stderr(contains("provenance init"));
        assert!(!directory.path().join(".provenance/cache").exists());
    }
}
