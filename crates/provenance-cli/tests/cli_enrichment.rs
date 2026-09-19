use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;

#[allow(clippy::too_many_lines)]
#[test]
fn cli_creates_and_exports_enriched_sources_and_resolutions() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();
    let export_path = dir.path().join("export.json");
    let export_path = export_path.to_string_lossy().to_string();

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
            "source_sah_2025",
            "--name",
            "Support at Home 2025",
            "--source-type",
            "legislation",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources", "create", "--repo", &repo, "--scope", "default", "--stdin", "--format",
            "json",
        ])
        .write_stdin(
            serde_json::json!({
                "id": "source_sah",
                "name": "Support at Home",
                "source_type": "legislation",
                "reference": "Department guidance",
                "commit_pin": "5e1f2a9c4b6d8e0f1234567890abcdef12345678",
                "effective_date": 1_714_521_600_000_i64,
                "review_date": 1_717_200_000_000_i64,
                "supersedes": ["source_sah_2025"]
            })
            .to_string(),
        )
        .assert()
        .success()
        .stdout(predicates::str::contains(
            r#""effective_date": 1714521600000"#,
        ))
        .stdout(predicates::str::contains(
            r#""commit_pin": "5e1f2a9c4b6d8e0f1234567890abcdef12345678""#,
        ))
        .stdout(predicates::str::contains(r#""supersedes": ["#))
        .stdout(predicates::str::contains(r#""source_sah_2025""#));
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

    let exported = std::fs::read_to_string(&export_path).unwrap();
    assert!(exported.contains(r#""commit_pin": "5e1f2a9c4b6d8e0f1234567890abcdef12345678""#));
    assert!(exported.contains(r#""effective_date": 1714521600000"#));
    assert!(exported.contains(r#""review_date": 1717200000000"#));

    // The resolutions anchor on a Requirement, whose creation enrolls it in
    // the review journal; from then on the scope refuses a lossy export.
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
            "req_sah",
            "--statement",
            "Support at Home shall be traceable",
            "--format",
            "json",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "resolutions",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "res_sah_2025",
            "--title",
            "SAH extraction, revised",
            "--requirement-id",
            "req_sah",
            "--position",
            "Revised extraction",
            "--rationale",
            "Reviewed",
            "--status",
            "draft",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "resolutions",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--stdin",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::json!({
                "id": "res_sah",
                "title": "SAH extraction",
                "requirement_ids": ["req_sah"],
                "supersedes": ["res_sah_2025"],
                "position": "Keep as draft extraction",
                "rationale": "Needs human review",
                "status": "draft",
                "context": "Codebase scan",
                "enforcement": "specification",
                "confidence": 0.91,
                "inputs": [{
                    "input_type": "regulatory",
                    "reference": "SAH program manual",
                    "summary": "Program rules reviewed"
                }],
                "made_by": "Analyst One",
                "approved_by": "Approver Two",
                "approved_at": 1_714_780_800_000_i64
            })
            .to_string(),
        )
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""input_type": "regulatory""#))
        .stdout(predicates::str::contains(r#""made_by": "Analyst One""#))
        .stdout(predicates::str::contains(r#""supersedes": ["#))
        .stdout(predicates::str::contains(r#""res_sah_2025""#));

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
            export_path.as_str(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "review-bearing scopes require lossless import/export support",
        ));
}

#[test]
fn cli_rejects_invalid_source_commit_pin() {
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
            "source_codebase",
            "--name",
            "Codebase",
            "--source-type",
            "project_artifact",
            "--commit-pin",
            "main",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid_commit_pin"));
}

/// An input has to name where it came from and say what it told the decision.
/// The create path refuses a blank field rather than storing an input that
/// locates nothing.
#[test]
#[verifies("rule_resolution_input_content", examples)]
fn cli_rejects_a_resolution_input_with_a_blank_reference() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();
    init_repo(&repo);

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
            "req_sah",
            "--statement",
            "SAH applies",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "resolutions",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--stdin",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::json!({
                "id": "res_sah",
                "requirement_ids": ["req_sah"],
                "title": "SAH extraction",
                "position": "Keep as draft extraction",
                "rationale": "Needs human review",
                "status": "draft",
                "supersedes": [],
                "inputs": [{
                    "input_type": "regulatory",
                    "reference": "   ",
                    "summary": "Program rules reviewed"
                }]
            })
            .to_string(),
        )
        .assert()
        .failure()
        .stderr(predicates::str::contains("request.inputs[0]"));
}

/// The same refusal on the way in from a file. An export edited to blank an
/// input summary does not import, so the hole cannot be walked around by
/// writing JSON instead of running `resolutions create`.
#[test]
#[verifies("rule_resolution_input_content", examples)]
fn cli_rejects_an_imported_resolution_input_with_a_blank_summary() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();
    let import_path = dir.path().join("scope.json");
    init_repo(&repo);

    // The scope file carries a resolution whose input summary was blanked, as
    // a hand-edited export would have it.
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
                "statement": "SAH applies",
                "status": "discovery"
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
                "supersedes": [],
                "inputs": [{
                    "input_type": "regulatory",
                    "reference": "SAH program manual",
                    "summary": ""
                }]
            }],
            "rules": [],
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
            import_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "resolution input summary must not be blank",
        ));
}

fn init_repo(repo: &str) {
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
