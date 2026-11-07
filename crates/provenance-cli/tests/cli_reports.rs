use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

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
            "source_schads",
            "create",
            "--type",
            "source",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--name",
            "SCHADS Award",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "req_schads_overtime",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--statement",
            "Overtime must follow SCHADS thresholds",
        ])
        .assert()
        .success();
    let store = provenance_store::repository::Repository::open(&repo, "default")
        .unwrap()
        .store();
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
            "res_schads_overtime",
            "create",
            "--type",
            "resolution",
            "--status",
            "approved",
            "--repo",
            &repo,
            "--scope",
            "default",
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
            "rule_schads_pay_001",
            "create",
            "--type",
            "rule",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--requirement-id",
            "req_schads_overtime",
            "--resolution-id",
            "res_schads_overtime",
            "--statement",
            "Pay overtime after the threshold",
            "--status",
            "active",
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
            "api",
            "sources/source_schads",
            "--query",
            "query=impact",
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
            "check", "--gaps", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "rules_with_complete_traceability",
        ))
        .stdout(predicates::str::contains("source_linked_requirements"))
        .stdout(predicates::str::contains("\"rule_id\"").not());
}
