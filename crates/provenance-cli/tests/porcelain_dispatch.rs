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
#[verifies("rule_porcelain_get_is_default_action", examples)]
fn an_omitted_get_action_accepts_view_options_and_global_option_positions() {
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
            "req_omitted_options",
            "--statement",
            "Omitted get actions retain their options.",
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
            "rule_omitted_options",
            "--requirement-id",
            "req_omitted_options",
            "--statement",
            "The child is returned.",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "--repo",
            &repo,
            "req_omitted_options",
            "--view",
            "children",
            "--depth",
            "2",
            "--kind",
            "rule",
            "--format",
            "json",
            "--limit",
            "7",
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
    assert_eq!(value["related"][0]["id"], "rule_omitted_options");
    assert_eq!(value["related"][0]["depth"], 1);
    assert_eq!(value["bounds"]["limit"], 7);
    assert_eq!(value["bounds"]["max_depth"], 2);
}

#[test]
fn cli_get_rejects_an_unknown_returned_kind() {
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
            "req_missing",
            "--repo",
            &repo,
            "--view",
            "children",
            "--kind",
            "invented",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported read options"));
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
