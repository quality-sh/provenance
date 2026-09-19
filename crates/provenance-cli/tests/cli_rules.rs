//! The generated Rule CLI reads the resource catalog.
use assert_cmd::Command;
use predicates::{prelude::PredicateBooleanExt, str::contains};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn repo_with_rule() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().to_str().unwrap();
    provenance()
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
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--id",
            "req_overtime",
            "--statement",
            "Overtime is paid",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            repo,
            "--id",
            "rule_overtime",
            "--requirement-id",
            "req_overtime",
            "--statement",
            "Pay overtime after the threshold",
            "--severity",
            "high",
        ])
        .assert()
        .success();
    tmp
}

#[test]
fn rule_list_and_member_read_use_v2_envelopes() {
    let tmp = repo_with_rule();
    let repo = tmp.path().to_str().unwrap();
    provenance()
        .args(["rules", "list", "--repo", repo, "--format", "json"])
        .assert()
        .success()
        .stdout(contains("\"items\"").and(contains("rule_overtime")));
    provenance()
        .args([
            "rules",
            "rule_overtime",
            "get",
            "--repo",
            repo,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("Pay overtime after the threshold"));
}

#[test]
fn missing_rule_returns_the_typed_read_failure() {
    let tmp = repo_with_rule();
    let repo = tmp.path().to_str().unwrap();
    provenance()
        .args(["rules", "rule_absent", "get", "--repo", repo])
        .assert()
        .failure()
        .stderr(contains("resource_not_found"));
}

#[test]
fn catalog_commands_offer_help_at_each_address() {
    for command in [
        vec!["rules", "--help"],
        vec!["rules", "create", "--help"],
        vec!["rules", "rule_a", "update", "--help"],
    ] {
        provenance()
            .args(command)
            .assert()
            .success()
            .stdout(contains("Catalog commands for rules"));
    }
}
