use assert_cmd::Command;

#[test]
fn requirements_list_identifies_the_selected_malformed_manifest() {
    let temporary = tempfile::tempdir().unwrap();
    let selected = temporary.path().join("selected");
    let manifest = selected.join(".provenance").join("state").join("manifest.json");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let malformed = b"{bad";
    std::fs::write(&manifest, malformed).unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(temporary.path())
        .args(["requirements", "list", "--repo", selected.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(manifest.to_str().unwrap()), "{stderr}");
    assert!(
        stderr.contains("key must be a string at line 1 column 2"),
        "{stderr}"
    );
    assert_eq!(std::fs::read(&manifest).unwrap(), malformed);
    assert!(!selected.join(".provenance/cache/provenance.db").exists());
    assert!(!selected.join(".provenance/state/scopes").exists());
}

#[test]
fn init_identifies_the_selected_malformed_manifest() {
    let temporary = tempfile::tempdir().unwrap();
    let selected = temporary.path().join("selected");
    let manifest = selected.join(".provenance").join("state").join("manifest.json");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let malformed = b"{bad";
    std::fs::write(&manifest, malformed).unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(temporary.path())
        .args(["init", "--path", selected.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(manifest.to_str().unwrap()), "{stderr}");
    assert!(
        stderr.contains("key must be a string at line 1 column 2"),
        "{stderr}"
    );
    assert_eq!(std::fs::read(&manifest).unwrap(), malformed);
    assert!(!selected.join(".provenance/cache/provenance.db").exists());
    assert!(!selected.join(".provenance/state/scopes").exists());
}

#[test]
fn init_keeps_schema_validation_wording() {
    let temporary = tempfile::tempdir().unwrap();
    let selected = temporary.path().join("selected");
    let manifest = selected.join(".provenance/state/manifest.json");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    let unsupported = serde_json::json!({"schema_version": 999, "scopes": []}).to_string();
    std::fs::write(&manifest, &unsupported).unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(temporary.path())
        .args(["init", "--path", selected.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("manifest schema_version must be"),
        "{stderr}"
    );
    assert!(!stderr.contains("failed to parse manifest"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), unsupported);
}
