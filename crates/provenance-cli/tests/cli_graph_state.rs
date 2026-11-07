use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::Path;

fn command(root: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.current_dir(root).args(args);
    command.assert()
}

fn write_manifest(root: &Path) {
    let state = root.join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("manifest.json"),
        serde_json::json!({
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
    write_manifest(root);
    let state = root.join(".provenance/state");
    let cache = root.join(".provenance/cache");
    let transaction = cache.join("import-transactions/interrupted");
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::rename(&state, transaction.join("backup-state")).unwrap();
    std::fs::write(
        cache.join("import-publication.json"),
        serde_json::json!({
            "transaction_dir": transaction,
            "phase": "backup_created"
        })
        .to_string(),
    )
    .unwrap();

    command(root, &["check", "--gaps"]).success();
    assert!(state.join("manifest.json").is_file());
    assert!(!cache.join("import-publication.json").exists());
}

#[test]
fn gaps_check_rejects_invalid_manifests() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("manifest.json"), "{").unwrap();
    command(directory.path(), &["check", "--gaps"])
        .failure()
        .stderr(contains("EOF"));
}

#[test]
fn gaps_check_rejects_a_manifest_without_scopes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let state = root.join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("manifest.json"),
        serde_json::json!({"scopes": []}).to_string(),
    )
    .unwrap();

    command(root, &["check", "--gaps"])
        .failure()
        .stderr(contains("manifest must contain at least one scope"));

    write_manifest(root);
    command(root, &["check", "--gaps"]).success();
}

#[test]
fn gaps_check_checks_repository_initialization_before_scope_validation() {
    let directory = tempfile::tempdir().unwrap();

    command(
        directory.path(),
        &["check", "--gaps", "--scope", "not valid"],
    )
    .failure()
    .stderr(contains("provenance init"));
}

#[test]
fn stale_publication_lock_does_not_create_recovery_state() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let locks = root.join(".provenance/cache/locks");
    std::fs::create_dir_all(&locks).unwrap();
    let lock = locks.join("repository.publication.lock");
    std::fs::write(&lock, "existing lock").unwrap();

    command(root, &["check", "--gaps"])
        .failure()
        .stderr(contains("provenance init"));
    assert_eq!(std::fs::read(&lock).unwrap(), b"existing lock");
    assert!(!root.join(".provenance/cache/import-transactions").exists());
    assert!(!root.join(".provenance/state").exists());
}

#[test]
fn a_manifest_directory_is_invalid_state() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join(".provenance/state/manifest.json")).unwrap();
    command(directory.path(), &["check", "--gaps"])
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
    command(directory.path(), &["check", "--gaps"])
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

#[test]
fn a_composed_check_reports_each_unavailable_category() {
    let directory = tempfile::tempdir().unwrap();
    let output = command(
        directory.path(),
        &["check", "--graph", "--statements", "--format", "json"],
    )
    .failure()
    .get_output()
    .stdout
    .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let categories = report["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0]["category"], "graph");
    assert_eq!(categories[1]["category"], "statements");
    for category in categories {
        assert_eq!(category["status"], "unavailable");
        assert!(category["unavailable_reason"]
            .as_str()
            .unwrap()
            .contains("provenance init"));
    }
    assert!(!directory.path().join(".provenance/cache").exists());
}
