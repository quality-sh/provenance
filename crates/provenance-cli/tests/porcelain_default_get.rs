//! The omitted-action get gate: when the CLI reads by default, which
//! arguments it accepts, and how reserved commands and unknown kinds behave.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use provenance_macros::verifies;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
#[verifies("rule_porcelain_get_is_default_action", examples)]
fn a_non_command_record_id_uses_get_when_the_action_is_omitted() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_bare_get",
            "--name",
            "Bare get",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["source_bare_get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "source_bare_get");
}

#[test]
fn a_bare_get_lookup_error_is_returned_without_command_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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

    provenance()
        .args(["missing_record", "--repo", &repo])
        .assert()
        .failure()
        .stderr(predicates::str::contains("record does not exist"))
        .stderr(predicates::str::contains("unrecognized subcommand").not());
}

#[test]
fn the_omitted_action_gate_accepts_only_supported_get_options() {
    let supported = ["--view", "children", "--depth", "2", "--limit", "7"];
    let arguments = ["provenance".to_owned(), "req_alpha".to_owned()]
        .iter()
        .chain(supported.iter())
        .cloned()
        .collect::<Vec<_>>();

    assert!(provenance_cli::porcelain::bare_target_selects_get(
        &arguments, false
    ));
    assert!(!provenance_cli::porcelain::bare_target_selects_get(
        &arguments, true
    ));

    let unknown = [
        "provenance".to_owned(),
        "req_alpha".to_owned(),
        "--wat".to_owned(),
    ];
    assert!(!provenance_cli::porcelain::bare_target_selects_get(
        &unknown, false
    ));
}

#[test]
#[verifies("rule_porcelain_get_selects_child_context", examples)]
fn omitted_action_get_accepts_the_supported_get_options() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--id",
            "req_options",
            "--statement",
            "Omitted actions keep their read options.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--id",
            "rule_options",
            "--requirement-id",
            "req_options",
            "--statement",
            "The child rule stays reachable.",
            "--severity",
            "high",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "req_options",
            "--view",
            "children",
            "--depth",
            "2",
            "--kind",
            "rule",
            "--limit",
            "7",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["view"], "children");
    assert_eq!(value["bounds"]["limit"], 7);
    assert_eq!(value["bounds"]["max_depth"], 2);
    let related = value["related"].as_array().unwrap();
    assert!(
        related.iter().any(
            |record| record["record"]["id"] == "rule_options" || record["id"] == "rule_options"
        ),
        "{value}"
    );
}

#[test]
#[verifies("rule_porcelain_get_has_grounding_impact", examples)]
fn omitted_action_get_accepts_global_options_before_the_target() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_grounding",
            "--name",
            "Grounding target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "--repo",
            &repo,
            "--format",
            "json",
            "source_grounding",
            "--view",
            "grounding",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["view"], "grounding");
}

#[test]
fn a_reserved_command_still_wins_over_get_shaped_options() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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

    provenance()
        .args(["check", "--view", "children", "--repo", &repo])
        .assert()
        .failure()
        .stderr(predicates::str::contains("record does not exist").not());
}

#[test]
fn an_unknown_returned_kind_is_rejected_instead_of_returning_empty() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_kind_gate",
            "--name",
            "Kind gate",
        ])
        .assert()
        .success();

    provenance()
        .args([
            "source_kind_gate",
            "get",
            "--view",
            "children",
            "--kind",
            "dinosaur",
            "--repo",
            &repo,
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported read options"));
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn explicit_get_reads_a_record_that_matches_a_builtin_command() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
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
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "check",
            "--name",
            "Command-shaped target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["check", "get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "check");
    assert_eq!(value["record"]["kind"], "source");
}
