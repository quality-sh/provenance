use assert_cmd::Command;

#[allow(clippy::too_many_lines)]
fn seed_repo() -> (tempfile::TempDir, String) {
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
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "domains",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "domain_payroll",
            "--name",
            "Payroll",
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
            "--domain-id",
            "domain_payroll",
        ])
        .assert()
        .success();
    let store = provenance_store::state_store::StateStore::new(
        provenance_store::layout::ProvenanceLayout::new(&repo),
    );
    let etag = store
        .requirement_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            &provenance_core::StableId::new("req_schads_overtime").unwrap(),
        )
        .unwrap()
        .etag;
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "req_schads_overtime",
            "update",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--if-match",
            &etag,
            "--stdin",
        ])
        .write_stdin(r#"{"relationships":{"cites":[{"source_id":"source_schads"}]}}"#)
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
            "res_schads_overtime",
            "--title",
            "Overtime interpretation",
            "--requirement-id",
            "req_schads_overtime",
            "--position",
            "Use award threshold",
            "--rationale",
            "Matches source clause",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "rule_schads_pay_001",
            "--requirement-id",
            "req_schads_overtime",
            "--resolution-id",
            "res_schads_overtime",
            "--statement",
            "Pay overtime after the threshold",
            "--severity",
            "high",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["materialize", "--repo", &repo, "--format", "json"])
        .assert()
        .success();
    (dir, repo)
}

#[test]
fn cli_reports_emit_json_shapes_for_impact_health_and_orphans() {
    let (_dir, repo) = seed_repo();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "source_schads",
            "impact",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("rule_schads_pay_001"))
        .stdout(predicates::str::contains("affected_rules"));
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "health", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("with_complete_traceability"))
        .stdout(predicates::str::contains("source_linked_requirements"));
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "orphans", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));
}
