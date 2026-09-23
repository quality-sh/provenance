//! Pins the import boundary: a document field the typed write cannot carry
//! is refused before anything is published, and a refusal leaves the stored
//! scope byte for byte unchanged.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

fn init(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success();
}

fn create_source(repo: &Path, id: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--name",
            "Policy",
            "--source-type",
            "policy",
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn export(repo: &Path, output: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
}

fn import(repo: &Path, input: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
}

/// Every file under the stored state, with its exact bytes.
fn stored_state(repo: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(directory: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                walk(&entry, files);
            } else {
                files.push((entry.clone(), std::fs::read(&entry).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(&repo.join(".provenance"), &mut files);
    files
}

#[test]
fn import_refuses_an_unknown_record_field_and_leaves_the_stored_state_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    create_source(&repo, "source_one");
    let export_path = dir.path().join("export.json");
    export(&repo, &export_path);

    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export_path).unwrap()).unwrap();
    document["sources"][0]["extension"] = serde_json::json!({"owner": "newer-tool"});
    document["sources"][0]["name"] = serde_json::json!("Renamed Policy");
    let incoming = dir.path().join("incoming.json");
    std::fs::write(&incoming, serde_json::to_vec(&document).unwrap()).unwrap();

    let before = stored_state(&repo);
    let failure = import(&repo, &incoming).failure();
    let stderr = String::from_utf8(failure.get_output().stderr.clone()).unwrap();

    assert!(
        stderr.contains("unknown field `sources.0.extension`"),
        "the refusal must name the field it cannot carry: {stderr}"
    );
    assert_eq!(
        stored_state(&repo),
        before,
        "a refused import must leave every stored file unchanged"
    );
}

#[test]
fn import_accepts_the_same_document_without_the_unknown_field() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    create_source(&repo, "source_one");
    let export_path = dir.path().join("export.json");
    export(&repo, &export_path);

    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export_path).unwrap()).unwrap();
    document["sources"][0]["name"] = serde_json::json!("Renamed Policy");
    let incoming = dir.path().join("incoming.json");
    std::fs::write(&incoming, serde_json::to_vec(&document).unwrap()).unwrap();

    import(&repo, &incoming).success();

    let shard =
        std::fs::read_to_string(repo.join(".provenance/state/scopes/default/sources/source.jsonl"))
            .unwrap();
    assert!(shard.contains("Renamed Policy"), "{shard}");
}

#[test]
fn import_refuses_trailing_input_and_leaves_the_stored_state_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    create_source(&repo, "source_one");
    let export_path = dir.path().join("export.json");
    export(&repo, &export_path);

    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export_path).unwrap()).unwrap();
    document["sources"][0]["name"] = serde_json::json!("Renamed Policy");
    let before = stored_state(&repo);
    let incoming = dir.path().join("incoming.json");

    for trailing in [" {}", " junk"] {
        std::fs::write(&incoming, format!("{}{trailing}", document)).unwrap();
        import(&repo, &incoming).failure();
        assert_eq!(
            stored_state(&repo),
            before,
            "trailing input {trailing:?} must leave every stored file unchanged"
        );
    }
}

#[test]
fn import_still_accepts_the_legacy_disposition_alias() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    let seed = dir.path().join("seed.json");
    std::fs::write(
        &seed,
        serde_json::json!({
            "scope": "default",
            "sources": [],
            "requirements": [],
            "resolutions": [],
            "rules": [],
            "threads": [],
            "messages": [],
            "promotion_decisions": []
        })
        .to_string(),
    )
    .unwrap();

    import(&repo, &seed).success();
}
