use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use provenance_macros::verifies;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
fn bare_help_flag_runs_before_a_repository_exists() {
    let directory = tempfile::tempdir().unwrap();

    provenance()
        .current_dir(directory.path())
        .arg("--help")
        .assert()
        .success();
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn target_first_get_runs_through_the_cli_entrypoint() {
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
            "source_porcelain_entrypoint",
            "--name",
            "Porcelain entrypoint",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "source_porcelain_entrypoint",
            "get",
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
    assert_eq!(value["record"]["id"], "source_porcelain_entrypoint");
    assert_eq!(value["record"]["kind"], "source");
    assert_eq!(value["view"], "record");
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn target_first_get_accepts_global_options_before_the_target() {
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
            "source_global_options",
            "--name",
            "Global options",
        ])
        .assert()
        .success();

    provenance()
        .args([
            "--repo",
            &repo,
            "--format",
            "json",
            "source_global_options",
            "get",
        ])
        .assert()
        .success();
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn explicit_get_disambiguates_a_target_that_matches_a_legacy_command() {
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
            "sources",
            "--name",
            "Reserved target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["sources", "get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "sources");
}

#[test]
fn a_builtin_command_takes_precedence_over_a_matching_record_id() {
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
        .args(["check", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["categories"].is_array(), "{value}");
}
