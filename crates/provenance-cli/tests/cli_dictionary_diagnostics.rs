use assert_cmd::Command;

#[test]
fn cache_setup_failure_reports_its_cause_without_claiming_download_attempts() {
    let temporary = tempfile::tempdir().unwrap();
    let cache_parent = temporary.path().join("cache-parent");
    std::fs::write(&cache_parent, b"file, not a directory").unwrap();
    let asset_dir = cache_parent.join("assets");
    let repo = temporary.path().join("repo");

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--quiet",
        ])
        .env("PROVENANCE_STE100_ASSET_DIR", &asset_dir)
        .env("PROVENANCE_STE100_INDEX_DIR", temporary.path().join("indexes"))
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("create the shared STE asset cache: "), "{stderr}");
    assert!(!stderr.contains("after 3 attempts"), "{stderr}");
    assert!(stderr.contains("Initialization continues without a dictionary"));
    assert!(!repo.join(".provenance/state/dictionary.json").exists());
}
