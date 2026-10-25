//! Action help on an existing record lists the fields that the action
//! accepts for that record type, and a Discussion action without a required
//! option is a usage error.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
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
            "--disposition-actor-id",
            "reviewer",
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
            "req_a",
            "--statement",
            "The system records topics.",
        ])
        .assert()
        .success();
    (directory, repo)
}

fn help(repo: &str, args: &[&str]) -> String {
    let output = provenance()
        .args(args)
        .args(["--repo", repo, "--help"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn update_help_lists_only_the_fields_of_the_existing_record_type() {
    let (_directory, repo) = init();

    let text = help(&repo, &["req_a", "update"]);
    assert!(text.contains("--statement"), "{text}");
    assert!(text.contains("--if-match"), "{text}");
    for flag in ["--url", "--expected-etag", "--position", "--made-by"] {
        assert!(!text.contains(flag), "{flag} in\n{text}");
    }
}

#[test]
fn header_bound_body_fields_stay_out_of_help_and_are_usage_errors() {
    let (_directory, repo) = init();

    let text = help(&repo, &["requirements", "req_a", "update"]);
    assert!(text.contains("--if-match"), "{text}");
    assert!(!text.contains("--expected-etag"), "{text}");
    provenance()
        .args([
            "requirements",
            "req_a",
            "update",
            "--repo",
            &repo,
            "--expected-etag",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ])
        .assert()
        .code(2);
}

#[test]
fn reply_help_marks_expected_version_required_and_its_absence_is_a_usage_error() {
    let (_directory, repo) = init();
    let output = provenance()
        .args([
            "req_a", "discuss", "--repo", &repo, "--body", "hello", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let started: Value = serde_json::from_slice(&output.stdout).unwrap();
    let discussion = started["receipt"]["discussion_id"].as_str().unwrap();

    let text = help(&repo, &[discussion, "reply"]);
    let usage = text
        .lines()
        .find(|line| line.starts_with("Usage:"))
        .unwrap_or_default();
    assert!(
        usage.contains("--expected-version <expected-version>"),
        "{text}"
    );
    assert!(text.contains("--body"), "{text}");
    assert!(!text.contains("--statement"), "{text}");
    provenance()
        .args([discussion, "reply", "--repo", &repo, "--body", "again"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--expected-version is required"));
}
