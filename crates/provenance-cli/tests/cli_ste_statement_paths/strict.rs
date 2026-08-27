use super::support::{
    create_requirement, create_rule, diagnostic, git, init, provenance, REQUIREMENTS_SHARD,
    RULES_SHARD,
};
use provenance_macros::verifies;
use serde_json::{json, Value};

fn commit(repository: &std::path::Path, message: &str) -> String {
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", message]);
    String::from_utf8(git(repository, &["rev-parse", "HEAD"]).stdout)
        .unwrap()
        .trim()
        .to_owned()
}

fn check_strict(repository: &std::path::Path, base: Option<&str>) -> std::process::Output {
    let mut command = provenance();
    command.args([
        "check",
        "--repo",
        repository.to_str().unwrap(),
        "--strict",
        "--format",
        "json",
    ]);
    if let Some(base) = base {
        command.args(["--base", base]);
    }
    command.output().unwrap()
}

fn rewrite_statement(repository: &std::path::Path, shard: &str, id: &str, statement: &str) {
    let path = repository.join(shard);
    let contents = std::fs::read_to_string(&path).unwrap();
    let rewritten = contents
        .lines()
        .map(|line| {
            let mut value: Value = serde_json::from_str(line).unwrap();
            if value["id"] == id {
                value["statement"] = json!(statement);
            }
            serde_json::to_string(&value).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(path, rewritten).unwrap();
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "--initial-branch", "main"]);
    git(repo, &["config", "user.email", "test@example.test"]);
    git(repo, &["config", "user.name", "Test"]);
    init(repo);
    directory
}

#[test]
#[verifies("rule_ste_strict_committed_statement_selection", examples)]
#[verifies("rule_ste_strict_committed_statement_gate", examples)]
fn strict_check_reads_the_git_head_candidate_on_a_clean_checkout() {
    let directory = repository();
    let repo = directory.path();
    create_requirement(repo, "req_changed", "Original statement");
    create_rule(repo, "rule_changed", "Original rule");
    let base = commit(repo, "base");
    rewrite_statement(repo, REQUIREMENTS_SHARD, "req_changed", "Café; changed");
    rewrite_statement(repo, RULES_SHARD, "rule_changed", "Rule; changed");
    let candidate = commit(repo, "direct JSONL statement edits");

    let output = check_strict(repo, None);

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report,
        json!({
            "status": "findings",
            "candidate_commit": candidate,
            "base_commit": base,
            "diagnostics": [
                diagnostic("requirement", "req_changed", 5),
                diagnostic("rule", "rule_changed", 4)
            ]
        })
    );
}

#[test]
#[verifies("rule_ste_strict_committed_statement_selection", examples)]
#[verifies("rule_ste_strict_committed_statement_gate", examples)]
fn explicit_base_can_check_findings_that_the_parent_policy_permits() {
    let directory = repository();
    let repo = directory.path();
    create_requirement(repo, "req_history", "Original statement");
    let clean_base = commit(repo, "clean base");
    rewrite_statement(repo, REQUIREMENTS_SHARD, "req_history", "History; finding");
    commit(repo, "statement finding");
    std::fs::write(repo.join("README.md"), "Unrelated candidate change.\n").unwrap();
    let candidate = commit(repo, "candidate");

    let parent_output = check_strict(repo, None);
    assert!(parent_output.status.success());
    let parent_report: Value = serde_json::from_slice(&parent_output.stdout).unwrap();
    assert_eq!(parent_report["status"], "ok");
    assert_eq!(parent_report["candidate_commit"], candidate);
    assert_eq!(parent_report["diagnostics"], json!([]));

    let explicit_output = check_strict(repo, Some(&clean_base));
    assert!(!explicit_output.status.success());
    let explicit_report: Value = serde_json::from_slice(&explicit_output.stdout).unwrap();
    assert_eq!(explicit_report["status"], "findings");
    assert_eq!(explicit_report["candidate_commit"], candidate);
    assert_eq!(explicit_report["base_commit"], clean_base);
    assert_eq!(
        explicit_report["diagnostics"],
        json!([diagnostic("requirement", "req_history", 7)])
    );
}

#[test]
#[verifies("rule_ste_strict_initial_commit_base", examples)]
#[verifies("rule_ste_strict_committed_statement_gate", examples)]
fn initial_commit_uses_an_empty_comparison_base() {
    let directory = repository();
    let repo = directory.path();
    create_requirement(repo, "req_initial", "Initial statement");
    rewrite_statement(repo, REQUIREMENTS_SHARD, "req_initial", "Initial; finding");
    let candidate = commit(repo, "initial commit");

    let output = check_strict(repo, None);

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "findings");
    assert_eq!(report["candidate_commit"], candidate);
    assert_eq!(report["base_commit"], Value::Null);
    assert_eq!(
        report["diagnostics"],
        json!([diagnostic("requirement", "req_initial", 7)])
    );
}

#[test]
#[verifies("rule_ste_strict_committed_statement_selection", examples)]
fn shallow_checkout_does_not_mistake_an_unavailable_parent_for_an_empty_base() {
    let scratch = tempfile::tempdir().unwrap();
    let origin = scratch.path().join("origin");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "--initial-branch", "main"]);
    git(&origin, &["config", "user.email", "test@example.test"]);
    git(&origin, &["config", "user.name", "Test"]);
    init(&origin);
    create_requirement(&origin, "req_history", "Original statement");
    rewrite_statement(
        &origin,
        REQUIREMENTS_SHARD,
        "req_history",
        "History; finding",
    );
    commit(&origin, "historical statement finding");
    std::fs::write(origin.join("README.md"), "Candidate change.\n").unwrap();
    commit(&origin, "candidate");
    let checkout = scratch.path().join("checkout");
    let source = format!("file://{}", origin.display());
    git(
        scratch.path(),
        &["clone", "--depth", "1", &source, checkout.to_str().unwrap()],
    );

    let output = check_strict(&checkout, None);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("first parent is not available. Fetch more Git history"));
}
