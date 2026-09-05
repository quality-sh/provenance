use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn import_replaces_only_target_scope_and_removes_all_stale_target_shards() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    add_other_scope(&repo);
    create_source(&repo, "default", "source_target");
    create_source(&repo, "other", "source_other");
    create_requirement(&repo, "default", "requirement_target");
    create_requirement(&repo, "other", "requirement_other");
    seed_stale_target_shards(&repo);

    let export = dir.path().join("replacement.json");
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
            export.to_str().unwrap(),
        ])
        .assert()
        .success();
    let mut replacement: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    replacement["sources"] = serde_json::json!([]);
    replacement["requirements"] = serde_json::json!([]);
    replacement["threads"] = serde_json::json!([]);
    replacement["messages"] = serde_json::json!([]);
    std::fs::write(&export, serde_json::to_vec(&replacement).unwrap()).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            export.to_str().unwrap(),
        ])
        .assert()
        .success();

    let state = repo.join(".provenance/state");
    assert_eq!(
        std::fs::read_to_string(state.join("scopes/default/sources/source.jsonl")).unwrap(),
        ""
    );
    assert!(
        std::fs::read_to_string(state.join("scopes/other/sources/source.jsonl"))
            .unwrap()
            .contains("source_other")
    );
    assert!(!state
        .join("scopes/default/ideation/landings.jsonl")
        .exists());
    assert!(!state.join("scopes/default/threads/2025-01.jsonl").exists());
    Command::cargo_bin("provenance")
        .unwrap()
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

fn init(repo: &std::path::Path) {
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

fn add_other_scope(repo: &std::path::Path) {
    let path = repo.join(".provenance/state/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "other", "path_prefix": "other"
        }));
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn create_source(repo: &std::path::Path, scope: &str, id: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            scope,
            "--id",
            id,
            "--name",
            id,
            "--source-type",
            "policy",
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn create_requirement(repo: &std::path::Path, scope: &str, id: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            scope,
            "--id",
            id,
            "--statement",
            id,
            "--status",
            "active",
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn seed_stale_target_shards(repo: &std::path::Path) {
    let scope = repo.join(".provenance/state/scopes/default");
    std::fs::create_dir_all(scope.join("ideation")).unwrap();
    std::fs::write(
        scope.join("ideation/landings.jsonl"),
        "{\"contributions\":[],\"synthesis_packets\":[],\"proposals\":[],\"assertions\":[],\"dispositions\":[]}\n",
    )
    .unwrap();
    std::fs::create_dir_all(scope.join("threads")).unwrap();
    std::fs::write(
        scope.join("threads/2025-01.jsonl"),
        format!("{{\"schema_version\":{},\"scope_id\":\"default\",\"id\":\"message_stale\",\"thread_id\":\"thread_stale\",\"role\":\"user\",\"body\":\"stale\",\"created_at\":1}}\n", SUPPORTED_SCHEMA_VERSION.0),
    )
    .unwrap();
}
