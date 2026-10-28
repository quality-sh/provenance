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
fn resolve_symbol_returns_matched_sites() {
    let tmp = repo_with_rule();
    let repo = tmp.path().to_str().unwrap();
    std::fs::write(
        tmp.path().join("code.rs"),
        concat!(
            "#[rule(\"rule_overtime\")]\n",
            "fn pay() {}\n",
            "#[verifies(\"rule_overtime\", examples)]\n",
            "fn checks_pay() {}\n",
        ),
    )
    .unwrap();
    let output = provenance()
        .args([
            "rules",
            "resolve-symbol",
            "--repo",
            repo,
            "--file",
            "code.rs",
            "--symbol",
            "pay",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let body: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let matches = body["data"]["matches"].as_array().unwrap();
    assert!(matches
        .iter()
        .any(|site| site["match_kind"] == "symbol" && site["item_name"] == "pay"));
    assert!(matches
        .iter()
        .any(|site| site["match_kind"] == "file" && site["item_name"] == "checks_pay"));
}
