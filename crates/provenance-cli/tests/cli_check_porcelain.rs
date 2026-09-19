use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

#[test]
fn cli_check_selector_runs_and_renders_only_the_selected_category() {
    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            directory.path().to_str().unwrap(),
            "--graph",
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
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["categories"].as_array().unwrap().len(), 1);
    assert_eq!(report["categories"][0]["category"], "graph");
    assert_eq!(report["categories"][0]["status"], "passed");
}

#[test]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn cli_check_defaults_to_readable_output() {
    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            directory.path().to_str().unwrap(),
            "--graph",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "graph: passed\n");
}

#[test]
fn cli_check_keeps_graph_findings_as_a_failing_exit() {
    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    std::fs::write(
        directory.path().join(".provenance/state/manifest.json"),
        "not a manifest",
    )
    .unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            directory.path().to_str().unwrap(),
            "--graph",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["categories"][0]["category"], "graph");
    assert_eq!(report["categories"][0]["status"], "findings");
}

#[test]
#[verifies("rule_porcelain_coverage_does_not_run_tests", examples)]
#[verifies("rule_porcelain_missing_binding_not_invalid", examples)]
fn cli_binding_check_reports_absence_without_running_or_failing_project_tests() {
    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    let marker = directory.path().join("project-tests-ran");
    std::fs::write(
        directory.path().join("project-test.sh"),
        format!("#!/bin/sh\ntouch {}\n", marker.display()),
    )
    .unwrap();
    let rules = directory
        .path()
        .join(".provenance/state/scopes/default/rules/rule.jsonl");
    std::fs::create_dir_all(rules.parent().unwrap()).unwrap();
    std::fs::write(
        rules,
        concat!(
            r#"{"schema_version":2,"scope_id":"default","id":"rule_unbound","statement":"The system keeps the record.","status":"active","severity":"medium","requirement_ids":[]}"#,
            "\n"
        ),
    )
    .unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(directory.path())
        .args(["check", "--repo", ".", "--bindings", "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["categories"][0]["category"], "bindings");
    assert_eq!(report["categories"][0]["status"], "findings");
    assert!(report["categories"][0]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("rule_unbound"))));
}

#[test]
fn cli_binding_check_keeps_the_repository_failure_policy() {
    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    let rules = directory
        .path()
        .join(".provenance/state/scopes/default/rules/rule.jsonl");
    std::fs::create_dir_all(rules.parent().unwrap()).unwrap();
    std::fs::write(
        rules,
        concat!(
            r#"{"schema_version":2,"scope_id":"default","id":"rule_unbound","statement":"The system keeps the record.","status":"active","severity":"medium","requirement_ids":[]}"#,
            "\n"
        ),
    )
    .unwrap();
    std::fs::write(
        directory.path().join(".provenance/settings.json"),
        r#"{"coverage":{"binding_findings":"error"}}"#,
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(directory.path())
        .args(["check", "--repo", ".", "--bindings", "--format", "json"])
        .assert()
        .failure();
}

#[test]
#[verifies("rule_porcelain_check_selector_union", examples)]
fn graph_selector_does_not_load_binding_policy() {
    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    std::fs::write(
        directory.path().join(".provenance/settings.json"),
        r#"{"coverage":{"binding_findings":"not-a-policy"}}"#,
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            directory.path().to_str().unwrap(),
            "--graph",
        ])
        .assert()
        .success();
}
