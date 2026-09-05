use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn cli_source_requirement_slice_materializes_and_reads_graph() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "source_schads",
            "--name",
            "SCHADS Award",
            "--source-type",
            "policy",
            "--url",
            "https://example.test/schads",
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
            &repo,
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
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "source-ref",
            "add",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--requirement-id",
            "req_schads_overtime",
            "--source-id",
            "source_schads",
            "--format",
            "json",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["materialize", "--repo", &repo, "--format", "json"])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "graph",
            "req_schads_overtime",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("source_schads"));
}

#[test]
fn cli_graph_preserves_enriched_v1_requirement_and_source_fields() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    let state = dir.path().join(".provenance/state");
    std::fs::create_dir_all(state.join("scopes/default/sources")).unwrap();
    std::fs::create_dir_all(state.join("scopes/default/requirements")).unwrap();
    std::fs::write(
        state.join("scopes/default/sources/source.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"source_sah","name":"Support at Home","source_type":"legislation","url":"https://example.test/sah","reference":"Department guidance"}).to_string(),
    )
    .unwrap();
    std::fs::write(
        state.join("scopes/default/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"req_sah","statement":"Support at Home shall be traceable","description":"Cloud import description","status":"discovery","source_refs":[{"source_id":"source_sah","clause":"Program overview"}]}).to_string(),
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args(["materialize", "--repo", &repo, "--format", "json"])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "graph", "req_sah", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""status": "discovery""#))
        .stdout(predicates::str::contains(r#""source_type": "legislation""#))
        .stdout(predicates::str::contains(
            r#""description": "Cloud import description""#,
        ))
        .stdout(predicates::str::contains(
            r#""reference": "Department guidance""#,
        ));
}
