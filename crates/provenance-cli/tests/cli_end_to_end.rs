use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn import_export_roundtrip_cli_exports_imports_checks_and_merges_local_state() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    let export_path = dir.path().join("export.json");
    let import_repo = dir.path().join("imported");
    let import_repo = import_repo.to_string_lossy().to_string();
    let export_path = export_path.to_string_lossy().to_string();

    init(&repo);
    create_graph(&repo);

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            &export_path,
        ])
        .assert()
        .success();

    init(&import_repo);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            &import_repo,
            "--scope",
            "default",
            "--input",
            &export_path,
            "--dry-run",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"records\""));
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            &import_repo,
            "--scope",
            "default",
            "--input",
            &export_path,
            "--format",
            "json",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["check", "--repo", &import_repo, "--format", "json"])
        .assert()
        .success();

    let base = dir.path().join("base.jsonl");
    let ours = dir.path().join("ours.jsonl");
    let theirs = dir.path().join("theirs.jsonl");
    std::fs::write(&base, "").unwrap();
    std::fs::write(&ours, "{\"id\":\"rule_b\"}\n").unwrap();
    std::fs::write(&theirs, "{\"id\":\"rule_a\"}\n").unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "merge-jsonl",
            &base.to_string_lossy(),
            &ours.to_string_lossy(),
            &theirs.to_string_lossy(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("rule_a"));
}

#[allow(clippy::too_many_lines)]
#[test]
fn import_export_roundtrip_preserves_enriched_v1_cloud_fields() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    let import_path = dir.path().join("enriched.json");
    let export_path = dir.path().join("export.json");
    let import_path = import_path.to_string_lossy().to_string();
    let export_path = export_path.to_string_lossy().to_string();

    init(&repo);
    std::fs::write(
        &import_path,
        serde_json::json!({
          "scope": "default",
          "sources": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "source_sah",
            "name": "Support at Home",
            "source_type": "legislation",
            "url": "https://example.test/sah",
            "reference": "Department guidance"
          }],
          "requirements": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "req_sah",
            "statement": "Support at Home shall be traceable",
            "description": "Cloud import description",
            "status": "discovery",
            "source_refs": [{"source_id": "source_sah", "clause": "Program overview"}]
          }],
          "resolutions": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "res_sah",
            "title": "SAH extraction",
            "position": "Keep as draft extraction",
            "rationale": "Needs human review",
            "status": "draft",
            "requirement_ids": ["req_sah"],
            "review_on": null,
            "context": "Codebase scan",
            "enforcement": "specification",
            "confidence": 0.91
          }],
          "rules": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "rule_sah_001",
            "name": "SAH rule",
            "description": "Rule description",
            "statement": "Draft rule shall stay draft",
            "status": "draft",
            "severity": "high",
            "requirement_ids": ["req_sah"],
            "resolution_ids": ["res_sah"],
            "source_document": "Example-API-main/src/example.php",
            "source_section": "lines 1-3"
          }],
          "edges": [],
          "threads": [],
          "messages": []
        })
        .to_string(),
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--input",
            &import_path,
            "--format",
            "json",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            &export_path,
        ])
        .assert()
        .success();

    let exported = std::fs::read_to_string(export_path).unwrap();
    assert!(exported.contains(&format!(
        r#""schema_version": {}"#,
        SUPPORTED_SCHEMA_VERSION.0
    )));
    assert!(exported.contains(r#""source_type": "legislation""#));
    assert!(exported.contains(r#""status": "discovery""#));
    assert!(exported.contains(r#""status": "draft""#));
    assert!(exported.contains(r#""source_refs""#));
    assert!(exported.contains(r#""source_document": "Example-API-main/src/example.php""#));
}

fn init(repo: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn create_graph(repo: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "source_schads",
            "--name",
            "SCHADS Award",
            "--format",
            "json",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "req_schads_overtime",
            "--statement",
            "Overtime must follow SCHADS thresholds",
            "--format",
            "json",
        ])
        .assert()
        .success();
}
