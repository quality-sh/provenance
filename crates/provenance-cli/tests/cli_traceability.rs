use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

#[allow(clippy::too_many_lines)]
#[test]
fn cli_traceability_chain_reports_rule_upstream_nodes_and_gaps() {
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
            "domain_payroll",
            "create",
            "--type",
            "domain",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--name",
            "Payroll",
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
            "--domain-id",
            "domain_payroll",
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
    // A second chain in the same scope, touching the rule nowhere. Its relations
    // are what `traceability` used to hand back alongside the real chain.
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "source_unrelated",
            "create",
            "--type",
            "source",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--name",
            "Unrelated Award",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "req_unrelated",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--statement",
            "Leave accrues monthly",
            "--domain-id",
            "domain_payroll",
        ])
        .assert()
        .success();
    let store = provenance_store::repository::Repository::open(&repo, "default")
        .unwrap()
        .store();
    let etag = store
        .requirement_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            &provenance_core::StableId::new("req_unrelated").unwrap(),
        )
        .unwrap()
        .etag;
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "req_unrelated",
            "update",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--if-match",
            &etag,
            "--stdin",
        ])
        .write_stdin(r#"{"relationships":{"cites":[{"source_id":"source_unrelated"}]}}"#)
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
        .stdout(predicates::str::contains(
            "\"relation\": \"requirement_ids\"",
        ))
        .stdout(predicates::str::contains("\"owner_type\": \"resolution\""));
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "traceability",
            "rule_schads_pay_001",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("source_schads"))
        .stdout(predicates::str::contains("res_schads_overtime"))
        // The other chain's records and relations stay out of a view of this rule.
        .stdout(predicates::str::contains("req_unrelated").not())
        .stdout(predicates::str::contains("source_unrelated").not());
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "gaps", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));
}
