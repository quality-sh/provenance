use super::support::{init_repo, provenance};
use provenance_core::SUPPORTED_SCHEMA_VERSION;

/// The macOS temp area sits behind `/var -> /private/var`, so a marker holds
/// an absolute path whose spelling matches neither the relative written
/// container nor the canonical one. An OS symlink above the repository must
/// not make a legitimate marker read as outside the cache; this reproduces
/// that shape on any Unix with an aliasing symlink above the repo.
#[test]
#[cfg(unix)]
fn relative_access_recovers_a_marker_written_through_a_path_alias() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("real")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("alias")).unwrap();
    let repo = dir.path().join("alias").join("repo");
    init_repo(&repo, None);
    let transaction = repo.join(".provenance/cache/import-transactions/completed");
    std::fs::create_dir_all(transaction.parent().unwrap()).unwrap();
    write_publication_marker(&repo, &transaction, "published");

    check_repo_relative(&repo);

    assert!(repo.join(".provenance/state/manifest.json").is_file());
    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
}

#[test]
fn relative_repository_access_restores_backup_after_interrupted_publication() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let transaction = repo.join(".provenance/cache/import-transactions/interrupted");
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::rename(
        repo.join(".provenance/state"),
        transaction.join("backup-state"),
    )
    .unwrap();
    write_publication_marker(&repo, &transaction, "backup_created");

    check_repo_relative(&repo);

    assert!(repo.join(".provenance/state/manifest.json").is_file());
    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
    assert!(!transaction.exists());
}

#[test]
fn relative_repository_access_clears_marker_when_transaction_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let transaction = repo.join(".provenance/cache/import-transactions/completed");
    std::fs::create_dir_all(transaction.parent().unwrap()).unwrap();
    write_publication_marker(&repo, &transaction, "published");

    check_repo_relative(&repo);

    assert!(repo.join(".provenance/state/manifest.json").is_file());
    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
}

#[test]
fn repository_access_finishes_cleanup_after_published_state() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let transaction = repo.join(".provenance/cache/import-transactions/published");
    std::fs::create_dir_all(transaction.join("backup-state")).unwrap();
    write_publication_marker(&repo, &transaction, "published");

    check_repo(&repo);

    assert!(repo.join(".provenance/state/manifest.json").is_file());
    assert!(!transaction.exists());
    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
}

fn check_repo(repo: &std::path::Path) {
    provenance()
        .args([
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn check_repo_relative(repo: &std::path::Path) {
    provenance()
        .current_dir(repo)
        .args(["check", "--repo", ".", "--format", "json"])
        .assert()
        .success();
}

fn write_publication_marker(repo: &std::path::Path, transaction: &std::path::Path, phase: &str) {
    std::fs::write(
        repo.join(".provenance/cache/import-publication.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": phase
        }))
        .unwrap(),
    )
    .unwrap();
}
