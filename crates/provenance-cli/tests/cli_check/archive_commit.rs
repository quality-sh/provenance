use super::{git, init, write_jsonl};
use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use std::path::Path;
use std::process::Output;

/// A Rule archived on a branch, then the branch lands as a squash commit on
/// main, so the archive commit is not in the history of HEAD.
#[test]
#[verifies("rule_archive_commit_in_head_history", examples)]
fn graph_check_reports_an_archive_commit_that_a_squash_merge_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    let base = commit(repo, "Base");
    init(repo);
    let statement = ["--statement", "The system is safe."];
    cli(repo, "requirements create --id req_safe", &statement);
    for id in ["rule_kept", "rule_dropped"] {
        let create = format!("{id} create --type rule --requirement-id req_safe");
        cli(repo, &create, &statement);
    }
    archive_rule(repo, "rule_kept", &base);
    git(repo, &["checkout", "--quiet", "-b", "feature"]);
    let branch = commit(repo, "Archive the rule on the branch");
    archive_rule(repo, "rule_dropped", &branch);
    let check = |format: &[&str]| run(repo, "check --graph", format);
    let on_branch = check(&[]);
    assert!(on_branch.status.success(), "{}", text(&on_branch));

    git(repo, &["checkout", "--quiet", "main"]);
    commit(repo, "Squash merge of the branch");

    let finding = format!("rule_dropped: archive commit {branch} is not in the history of HEAD");
    let readable = check(&[]);
    assert!(!readable.status.success());
    let readable = text(&readable);
    assert!(readable.contains(&finding), "{readable}");
    assert!(!readable.contains("rule_kept"), "{readable}");

    let json = check(&["--format", "json"]);
    assert!(!json.status.success());
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    let graph = &report["categories"][0];
    assert_eq!(graph["category"], "graph");
    assert_eq!(graph["status"], "findings");
    assert_eq!(
        graph["findings"],
        serde_json::json!([{ "message": finding }])
    );
}

/// A scoped graph check reads only the archive commits of its scope, so a
/// record that does not load in another scope gives no finding.
#[test]
#[verifies("rule_archive_commit_in_head_history", examples)]
fn scoped_graph_check_ignores_archive_commits_in_other_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    commit(repo, "Base");
    init(repo);
    let state = repo.join(".provenance/state");
    std::fs::write(
        state.join("manifest.json"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scopes":[{"id":"alpha","path_prefix":"."},{"id":"beta","path_prefix":"beta"}]}).to_string(),
    )
    .unwrap();
    write_jsonl(
        &state.join("scopes/beta/rules/rule.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"beta","id":"rule_beta","requirement_id":"req_beta","statement":"The system is safe.","status":"archived","archived_in_commit":{"commit":"not-a-commit"}}).to_string(),
    );

    let scoped = run(repo, "check --graph --scope alpha", &[]);
    assert!(scoped.status.success(), "{}", text(&scoped));
    let every_scope = run(repo, "check --graph", &[]);
    assert!(!every_scope.status.success(), "{}", text(&every_scope));
}

fn commit(repo: &Path, message: &str) -> String {
    git(
        repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            message,
        ],
    );
    let head = std::process::Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(head.stdout).unwrap().trim().to_owned()
}

/// Runs `provenance` with the words of `command`, then `extra`, on `repo`.
fn run(repo: &Path, command: &str, extra: &[&str]) -> Output {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(command.split_whitespace())
        .args(extra)
        .args(["--repo", repo.to_str().unwrap()])
        .output()
        .unwrap()
}

fn cli(repo: &Path, command: &str, extra: &[&str]) -> String {
    let output = run(repo, command, extra);
    assert!(
        output.status.success(),
        "provenance {command} failed: {}",
        text(&output)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn archive_rule(repo: &Path, id: &str, commit: &str) {
    let current = cli(repo, &format!("api rules/{id} --format json"), &[]);
    let current: serde_json::Value = serde_json::from_str(&current).unwrap();
    let etag = current["data"]["edit"]["etag"].as_str().unwrap();
    let stamp = serde_json::json!({ "commit": commit }).to_string();
    let update = format!("{id} update --if-match {etag} --status archived");
    cli(repo, &update, &["--archived-in-commit-json", &stamp]);
}
