use assert_cmd::Command;
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
            "source_sah",
            "--name",
            "Support at Home",
            "--source-type",
            "legislation",
            "--reference",
            "Department guidance",
            "--commit-pin",
            "5e1f2a9c4b6d8e0f1234567890abcdef12345678",
            "--effective-date",
            "1714521600000",
            "--review-date",
            "1717200000000",
            "--superseded-by",
            "source_sah_2025",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            r#""effective_date": 1714521600000"#,
        ))
        .stdout(predicates::str::contains(
            r#""commit_pin": "5e1f2a9c4b6d8e0f1234567890abcdef12345678""#,
        ))
        .stdout(predicates::str::contains(
            r#""superseded_by": "source_sah_2025""#,
        ));
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
            "res_sah",
            "--title",
            "SAH extraction",
            "--requirement-id",
            "req_sah",
            "--position",
            "Keep as draft extraction",
            "--rationale",
            "Needs human review",
            "--status",
            "draft",
            "--context",
            "Codebase scan",
            "--enforcement",
            "specification",
            "--confidence",
            "0.91",
            "--input-type",
            "regulatory",
            "--input-reference",
            "SAH program manual",
            "--input-summary",
            "Program rules reviewed",
            "--made-by",
            "Analyst One",
            "--approved-by",
            "Approver Two",
            "--approved-at",
            "1714780800000",
            "--superseded-by",
            "res_sah_2025",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""input_type": "regulatory""#))
        .stdout(predicates::str::contains(r#""made_by": "Analyst One""#))
        .stdout(predicates::str::contains(
            r#""superseded_by": "res_sah_2025""#,
        ));

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
    assert!(exported.contains(r#""commit_pin": "5e1f2a9c4b6d8e0f1234567890abcdef12345678""#));
    assert!(exported.contains(r#""effective_date": 1714521600000"#));
    assert!(exported.contains(r#""review_date": 1717200000000"#));
    assert!(exported.contains(r#""input_type": "regulatory""#));
    assert!(exported.contains(r#""approved_at": 1714780800000"#));
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
        .stderr(predicates::str::contains("commit pin must"));
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
            "--id",
            "res_sah",
            "--requirement-id",
            "req_sah",
            "--title",
            "SAH extraction",
            "--position",
            "Keep as draft extraction",
            "--rationale",
            "Needs human review",
            "--status",
            "draft",
            "--input-type",
            "regulatory",
            "--input-reference",
            "   ",
            "--input-summary",
            "Program rules reviewed",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "resolution input reference must not be blank",
        ));
}

/// The same refusal on the way in from a file. An export edited to blank an
/// input summary does not import, so the hole cannot be walked around by
/// writing JSON instead of running `resolutions create`.
#[test]
#[verifies("rule_resolution_input_content", examples)]
fn cli_rejects_an_imported_resolution_input_with_a_blank_summary() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();
    let export_path = dir.path().join("export.json");
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
            "--id",
            "res_sah",
            "--requirement-id",
            "req_sah",
            "--title",
            "SAH extraction",
            "--position",
            "Keep as draft extraction",
            "--rationale",
            "Needs human review",
            "--status",
            "draft",
            "--input-type",
            "regulatory",
            "--input-reference",
            "SAH program manual",
            "--input-summary",
            "Program rules reviewed",
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
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export_path).unwrap()).unwrap();
    exported["resolutions"][0]["inputs"][0]["summary"] = serde_json::json!("");
    std::fs::write(&export_path, serde_json::to_vec(&exported).unwrap()).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--input",
            export_path.to_str().unwrap(),
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
