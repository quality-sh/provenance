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
    create_edge(&repo, "default", "source_target", "requirement_target");
    create_edge(&repo, "other", "source_other", "requirement_other");
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
    replacement["edges"] = serde_json::json!([]);
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
    let edges = std::fs::read_dir(state.join("edges"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<String>();
    assert!(edges.contains("source_other"));
    assert!(!edges.contains("source_target"));
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

#[test]
fn import_rejects_edge_owned_by_another_declared_scope() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    add_other_scope(&repo);
    create_source(&repo, "other", "source_other");
    create_requirement(&repo, "other", "requirement_other");
    create_edge(&repo, "other", "source_other", "requirement_other");

    let other_export = dir.path().join("other.json");
    export_scope(&repo, "other", &other_export);
    let other: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&other_export).unwrap()).unwrap();

    let target_export = dir.path().join("default.json");
    export_scope(&repo, "default", &target_export);
    let mut target: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&target_export).unwrap()).unwrap();
    target["edges"] = other["edges"].clone();
    std::fs::write(&target_export, serde_json::to_vec(&target).unwrap()).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            target_export.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "edge scope_id must match import scope",
        ));
}

#[test]
fn import_preserves_unknown_fields_on_edges_from_other_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    add_other_scope(&repo);
    create_source(&repo, "other", "source_other");
    create_requirement(&repo, "other", "requirement_other");
    create_edge(&repo, "other", "source_other", "requirement_other");

    let edge_path = repo.join(".provenance/state/edges/edges-00.jsonl");
    let mut other_edge: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(&edge_path).unwrap().trim()).unwrap();
    other_edge["future_extension"] = serde_json::json!({"enabled": true});
    std::fs::write(
        &edge_path,
        format!("{}\n", serde_json::to_string(&other_edge).unwrap()),
    )
    .unwrap();

    let target_export = dir.path().join("default.json");
    export_scope(&repo, "default", &target_export);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            target_export.to_str().unwrap(),
        ])
        .assert()
        .success();

    let preserved_edges = std::fs::read_dir(repo.join(".provenance/state/edges"))
        .unwrap()
        .flat_map(|entry| {
            std::fs::read_to_string(entry.unwrap().path())
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(preserved_edges.contains(&other_edge));
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

fn create_edge(repo: &std::path::Path, scope: &str, source: &str, requirement: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "edges",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            scope,
            "--type",
            "references",
            "--from-type",
            "source",
            "--from-id",
            source,
            "--to-type",
            "requirement",
            "--to-id",
            requirement,
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn export_scope(repo: &std::path::Path, scope: &str, output: &std::path::Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            scope,
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
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
    std::fs::write(
        repo.join(".provenance/state/edges/edges-99.jsonl"),
        format!("{{\"schema_version\":{},\"scope_id\":\"default\",\"id\":\"references_source_source_target_to_requirement_requirement_target_stale\",\"edge_type\":\"references\",\"from_type\":\"source\",\"from_id\":\"source_target\",\"to_type\":\"requirement\",\"to_id\":\"requirement_target\"}}\n", SUPPORTED_SCHEMA_VERSION.0),
    )
    .unwrap();
}
