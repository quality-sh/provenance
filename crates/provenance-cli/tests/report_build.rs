//! Golden tests for the envelope builder: the producer that reads a real
//! repository at a named base and head and writes a schema-1 envelope from
//! live scan and graph facts. The builder never fabricates a verification
//! run, a pass, or a commit, and it labels what it cannot determine.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;
use std::process::Command as ProcessCommand;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn write(repo: &Path, relative: &str, contents: &str) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn provenance(repo: &Path) -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.current_dir(repo);
    command
}

fn create_requirement(repo: &Path, id: &str, statement: &str) {
    provenance(repo)
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--statement",
            statement,
        ])
        .assert()
        .success();
}

fn create_rule(repo: &Path, id: &str, requirement: &str, statement: &str) {
    provenance(repo)
        .args([
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--requirement-id",
            requirement,
            "--statement",
            statement,
        ])
        .assert()
        .success();
}

/// A real repository with two commits: the base commit holds one Requirement
/// and one active Rule; the head commit adds a Resolution, a second Rule and
/// the produced_by relation. The working tree stays clean at head.
fn two_commit_repo() -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_path_buf();
    let show = repo.to_str().unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    write(&repo, "src/lib.rs", "pub fn anchor() {}\n");
    provenance(&repo)
        .args([
            "init",
            "--path",
            show,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    create_requirement(&repo, "req_anchor", "The anchor requirement holds");
    create_rule(
        &repo,
        "rule_anchor",
        "req_anchor",
        "The anchor rule holds for the anchor requirement",
    );
    let base = commit(&repo, "base: anchor requirement and rule");

    create_rule(
        &repo,
        "rule_added",
        "req_anchor",
        "The added rule holds for the anchor requirement",
    );
    let head = commit(&repo, "head: added rule");

    assert!(base != head, "fixture must produce two distinct commits");
    (dir, base, head)
}

fn build_envelope(repo: &Path, base: &str, head: &str) -> Value {
    let output = provenance(repo)
        .args([
            "report",
            "build",
            "--repo",
            repo.to_str().unwrap(),
            "--base",
            base,
            "--head",
            head,
            "--repository",
            "quality-sh/provenance",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "report build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn graph_only_change_between_two_real_commits_lists_added_records() {
    let (_dir, base, head) = two_commit_repo();
    let repo = _dir.path();
    let envelope = build_envelope(repo, &base, &head);

    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["repository"], "quality-sh/provenance");
    assert_eq!(envelope["base_commit"], base);
    assert_eq!(envelope["head_commit"], head);
    assert_eq!(envelope["scan"]["completeness"], "complete");
    assert_eq!(envelope["scan"]["baseline"], "compatible");

    let changes = envelope["graph_changes"].as_array().unwrap();
    let added_rule = changes
        .iter()
        .find(|change| change["id"] == "rule_added")
        .expect("the added rule must appear as a graph change");
    assert_eq!(added_rule["kind"], "rule");
    assert_eq!(added_rule["change"], "added");
    assert_eq!(
        added_rule["statement"],
        "The added rule holds for the anchor requirement"
    );
    let serves = added_rule["relations_added"]
        .as_array()
        .unwrap()
        .iter()
        .find(|relation| relation["relation"] == "serves")
        .expect("the added rule must carry its serves relation");
    assert_eq!(serves["target_kind"], "requirement");
    assert_eq!(serves["target_id"], "req_anchor");

    // An unchanged record produces no diff row at all: the anchor rule must
    // not appear as added or changed between the two commits.
    assert!(
        !changes.iter().any(|change| change["id"] == "rule_anchor"
            && matches!(change["change"].as_str(), Some("added") | Some("changed"))),
        "the unchanged anchor rule must not be reported as changed"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn active_rule_without_verification_is_reported_and_no_run_is_invented() {
    let (_dir, base, head) = two_commit_repo();
    let repo = _dir.path();
    let envelope = build_envelope(repo, &base, &head);

    assert!(
        envelope["verification_runs"]
            .as_array()
            .map_or(true, |runs| runs.is_empty()),
        "the builder must never fabricate a verification run"
    );

    let findings = envelope["findings"].as_array().unwrap();
    let absence = findings
        .iter()
        .find(|finding| {
            finding["code"] == "active_rule_missing_verification"
                && finding["subject"]["id"] == "rule_added"
        })
        .expect("the new unverified rule must carry an absence finding");
    assert_eq!(absence["subject"]["kind"], "rule");
    assert_eq!(absence["binding_presence"], "absent");
    assert_eq!(absence["comparison"], "new");

    let anchor_absence = findings
        .iter()
        .find(|finding| {
            finding["code"] == "active_rule_missing_verification"
                && finding["subject"]["id"] == "rule_anchor"
        })
        .expect("the base rule without verification is pre-existing");
    assert_eq!(anchor_absence["comparison"], "pre_existing");
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn repeated_builds_of_the_same_state_are_byte_identical() {
    let (_dir, base, head) = two_commit_repo();
    let repo = _dir.path();
    let first = provenance(repo)
        .args([
            "report",
            "build",
            "--repo",
            repo.to_str().unwrap(),
            "--base",
            &base,
            "--head",
            &head,
            "--repository",
            "quality-sh/provenance",
        ])
        .output()
        .unwrap();
    let second = provenance(repo)
        .args([
            "report",
            "build",
            "--repo",
            repo.to_str().unwrap(),
            "--base",
            &base,
            "--head",
            &head,
            "--repository",
            "quality-sh/provenance",
        ])
        .output()
        .unwrap();
    assert!(first.status.success() && second.status.success());
    assert_eq!(
        first.stdout, second.stdout,
        "the same repository state must build a byte-identical envelope"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn missing_baseline_facts_are_labelled_honestly() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    write(&repo, "README.md", "fixture\n");
    let base = commit(&repo, "initial commit without provenance state");

    provenance(&repo)
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    create_requirement(&repo, "req_late", "The late requirement holds");
    let head = commit(&repo, "head: provenance state begins");

    let envelope = build_envelope(&repo, &base, &head);
    assert_eq!(envelope["scan"]["baseline"], "missing");
    assert!(
        envelope["graph_changes"]
            .as_array()
            .map_or(true, |changes| changes.is_empty()),
        "a missing baseline cannot establish graph changes"
    );
    let findings = envelope["findings"].as_array().cloned().unwrap_or_default();
    for finding in &findings {
        assert_eq!(
            finding["comparison"], "uncertain",
            "a missing baseline cannot label a finding new or pre-existing"
        );
    }
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn an_uncommitted_working_tree_is_labelled_incomplete_with_a_reason() {
    let (_dir, base, head) = two_commit_repo();
    let repo = _dir.path();
    write(repo, "src/lib.rs", "pub fn anchor() -> u32 { 1 }\n");

    let envelope = build_envelope(repo, &base, &head);
    assert_eq!(envelope["scan"]["completeness"], "incomplete");
    let reason = envelope["scan"]["incompleteness_reason"]
        .as_str()
        .expect("an incomplete scan must state its reason");
    assert!(
        !reason.trim().is_empty(),
        "the incompleteness reason must not be empty"
    );
    let findings = envelope["findings"].as_array().cloned().unwrap_or_default();
    for finding in &findings {
        assert_ne!(
            finding["binding_presence"], "absent",
            "an incomplete scan cannot establish absence"
        );
    }
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn builder_output_feeds_the_renderer_and_passes_its_checks() {
    let (_dir, base, head) = two_commit_repo();
    let repo = _dir.path();
    let envelope = build_envelope(repo, &base, &head);

    let envelope_path = _dir.path().join("envelope.json");
    std::fs::write(
        &envelope_path,
        serde_json::to_vec_pretty(&envelope).unwrap(),
    )
    .unwrap();

    let rendered = provenance(repo)
        .args(["report", "render", "--input"])
        .arg(&envelope_path)
        .output()
        .unwrap();
    assert!(
        rendered.status.success(),
        "the built envelope must pass the renderer's validate and duplicate checks: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    let markdown = String::from_utf8(rendered.stdout).unwrap();
    assert!(markdown.starts_with("# Provenance report"));
    assert!(
        markdown.contains("Next action:"),
        "catalog prescriptions must survive the producer boundary"
    );
    assert!(
        markdown.contains(&base),
        "the renderer must name the base commit from the envelope"
    );
}
