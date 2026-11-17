use super::support::{create_system_source, init_repo, provenance};

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

/// A stop after the marker leaves the change list in the transaction and a
/// put that is not installed yet. Recovery installs it.
#[test]
fn relative_repository_access_completes_an_interrupted_install() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    create_system_source(&repo);
    let sources = repo.join(".provenance/state/scopes/default/sources");
    let shard = std::fs::read_dir(&sources)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name()
        .into_string()
        .unwrap();
    let key = format!("scopes/default/sources/{shard}");
    let transaction = repo.join(".provenance/cache/import-transactions/interrupted");
    std::fs::create_dir_all(transaction.join("staged/scopes/default/sources")).unwrap();
    let staged = transaction.join("staged").join(&key);
    std::fs::rename(sources.join(&shard), &staged).unwrap();
    let digest: [u8; 32] =
        <sha2::Sha256 as sha2::Digest>::digest(std::fs::read(&staged).unwrap()).into();
    std::fs::write(
        transaction.join("changes.json"),
        serde_json::json!({ "puts": [{ "key": key, "sha256": digest }], "deletes": [] })
            .to_string(),
    )
    .unwrap();
    write_publication_marker(&repo, &transaction, "installing");

    check_repo_relative(&repo);

    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
    assert!(!transaction.exists());
    assert_eq!(source_ids(&repo), ["source_a"], "the install is complete");
}

/// A marker that names a phase that the program does not know does not
/// parse. The command refuses it with the path of the marker, states that
/// the program cannot read it, and changes no file.
#[test]
fn repository_access_refuses_a_marker_that_it_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let transaction = repo.join(".provenance/cache/import-transactions/unknown");
    std::fs::create_dir_all(&transaction).unwrap();
    write_publication_marker(&repo, &transaction, "backup_created");

    let output = provenance()
        .args(["check", "--repo", repo.to_str().unwrap()])
        .output()
        .unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    // The path of the marker has the separators of the platform, so the
    // test checks the text around it.
    assert!(stderr.starts_with("Error: read marker "), "{stderr}");
    assert!(
        stderr.contains(
            "import-publication.json: this program cannot read the content of the marker"
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("release"), "{stderr}");
    assert!(transaction.is_dir());
    assert!(repo.join(".provenance/state/manifest.json").is_file());
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
    std::fs::create_dir_all(transaction.join("staged")).unwrap();
    std::fs::copy(
        repo.join(".provenance/state/manifest.json"),
        transaction.join("staged/manifest.json"),
    )
    .unwrap();
    create_system_source(&repo);
    write_publication_marker(&repo, &transaction, "published");

    check_repo(&repo);

    assert!(repo.join(".provenance/state/manifest.json").is_file());
    assert!(!transaction.exists());
    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
    assert_eq!(
        source_ids(&repo),
        ["source_a"],
        "a published state stays live after the commit point"
    );
}

fn source_ids(repo: &std::path::Path) -> Vec<String> {
    provenance_store::repository::Repository::open(repo, "default")
        .unwrap()
        .store()
        .list_sources(&provenance_core::ScopeId::new("default").unwrap())
        .unwrap()
        .into_iter()
        .map(|source| source.id.as_str().to_owned())
        .collect()
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
            "transaction_dir": transaction,
            "phase": phase
        }))
        .unwrap(),
    )
    .unwrap();
}
