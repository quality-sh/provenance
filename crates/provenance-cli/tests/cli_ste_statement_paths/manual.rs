use super::support::{
    create_requirement, create_rule, diagnostic, git, init, provenance, REQUIREMENTS_SHARD,
    RULES_SHARD,
};
use provenance_macros::verifies;
use serde_json::{json, Value};

fn commit(repository: &std::path::Path, message: &str) {
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", message]);
}

fn check(repository: &std::path::Path) -> std::process::Output {
    provenance()
        .args([
            "check",
            "--repo",
            repository.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}

fn statements(report: &Value) -> &Value {
    report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["category"] == "statements")
        .unwrap()
}

fn diagnostics(report: &Value) -> Value {
    Value::Array(
        statements(report)["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["detail"].clone())
            .collect(),
    )
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

#[test]
#[verifies("rule_ste_manual_changed_statement_report", examples)]
fn check_reports_only_new_and_statement_changed_records_against_git_head() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "--initial-branch", "main"]);
    git(repo, &["config", "user.email", "test@example.test"]);
    git(repo, &["config", "user.name", "Test"]);
    init(repo);
    create_requirement(repo, "req_legacy", "Legacy statement");
    create_requirement(repo, "req_changed", "Original statement");
    commit(repo, "base");
    rewrite_statement(repo, REQUIREMENTS_SHARD, "req_legacy", "Legacy; invalid");
    commit(repo, "pin legacy invalid text");

    rewrite_statement(repo, REQUIREMENTS_SHARD, "req_changed", "Café; changed");
    create_rule(repo, "rule_added", "Rule added");
    rewrite_statement(repo, RULES_SHARD, "rule_added", "Rule; added");
    let output = check(repo);

    assert!(output.status.success(), "manual findings are informational");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(statements(&report)["status"], "findings");
    assert_eq!(
        diagnostics(&report),
        json!([
            diagnostic("requirement", "req_changed", 5),
            diagnostic("rule", "rule_added", 4)
        ])
    );
}

#[test]
#[verifies("rule_ste_manual_changed_statement_report", examples)]
fn clean_repository_check_is_successful_and_has_no_statement_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "--initial-branch", "main"]);
    git(repo, &["config", "user.email", "test@example.test"]);
    git(repo, &["config", "user.name", "Test"]);
    init(repo);
    create_requirement(repo, "req_clean", "Clean statement");
    commit(repo, "base");

    let first = check(repo);
    let second = check(repo);

    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    let report: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(statements(&report)["status"], "passed");
    assert_eq!(diagnostics(&report), json!([]));
}

#[test]
#[verifies("rule_ste_manual_changed_statement_report", examples)]
fn repository_without_git_head_reports_no_statement_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "--initial-branch", "main"]);
    init(repo);
    create_requirement(repo, "req_unborn", "Clean statement");
    rewrite_statement(repo, REQUIREMENTS_SHARD, "req_unborn", "Changed; statement");

    let output = check(repo);

    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(statements(&report)["status"], "passed");
    assert_eq!(diagnostics(&report), json!([]));
}
