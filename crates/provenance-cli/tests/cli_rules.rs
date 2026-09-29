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
fn rule_list_and_member_read_use_response_envelopes() {
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
fn rule_create_defaults_to_draft_and_accepts_explicit_active_status() {
    let tmp = repo_with_rule();
    let repo = tmp.path().to_str().unwrap();
    provenance()
        .args(["rules", "rule_overtime", "get", "--repo", repo])
        .assert()
        .success()
        .stdout(contains("\"status\":\"draft\""));

    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            repo,
            "--id",
            "rule_ratified",
            "--requirement-id",
            "req_overtime",
            "--statement",
            "A person ratified this Rule",
            "--status",
            "active",
        ])
        .assert()
        .success()
        .stdout(contains("\"status\":\"active\""));
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
    for (command, usage) in [
        (&["rules", "--help"][..], "rules create"),
        (
            &["rules", "create", "--help"][..],
            "provenance rules create",
        ),
        (
            &["rules", "rule_a", "update", "--help"][..],
            "provenance rules <id> update",
        ),
    ] {
        provenance()
            .args(command)
            .assert()
            .success()
            .stdout(contains(usage));
    }
}

#[test]
fn rule_create_help_shows_only_the_new_draft_default() {
    provenance()
        .args(["rules", "create", "--help"])
        .assert()
        .success()
        .stdout(contains(
            "--status <draft|review|active|deprecated|archived>",
        ))
        .stdout(contains("default: \"draft\""))
        .stdout(predicates::str::contains("default: \"active\"").not());
}
