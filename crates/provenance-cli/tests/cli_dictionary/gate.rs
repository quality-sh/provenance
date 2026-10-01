use crate::support::{
    create_requirement, error_json, imported_dictionary, init, provenance, write_reference,
    UNAPPROVED_WORD,
};
use provenance_macros::verifies;
use provenance_ste100::store_dictionary_index;
use serde_json::Value;
use std::path::Path;

fn statement() -> String {
    format!("The {UNAPPROVED_WORD} item stops.")
}

fn check_statement(repo: &Path, index_directory: &Path, statement: &str) -> Value {
    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
        .write_stdin(serde_json::json!({"statement": statement}).to_string())
        .args([
            "statement-checks",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "the preflight must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn setup_dictionary() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    init(&repo);
    let dictionary = imported_dictionary();
    store_dictionary_index(dictionary, &index_directory).unwrap();
    write_reference(&repo, dictionary);
    (scratch, repo, index_directory)
}

fn assert_named_rule_one_one_finding(report: &Value, word: &str) {
    let finding = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| {
            finding["rule"] == "1.1" && finding["message"].as_str().unwrap().contains(word)
        })
        .unwrap_or_else(|| panic!("the report must name {word} and Rule 1.1: {report}"));
    assert!(
        finding["message"].as_str().unwrap().contains("Rule 1.1"),
        "the finding must name the rule in plain text: {finding}"
    );
}

#[test]
fn rule_preflight_matches_the_typed_write_for_the_trial_statement() {
    let (_scratch, repo, index_directory) = setup_dictionary();
    let statement = "A claim amount must use AUD";
    let preflight = check_statement(&repo, &index_directory, statement);
    assert_named_rule_one_one_finding(&preflight["data"], "claim");

    create_requirement(
        &repo,
        &index_directory,
        "req_anchor",
        "The anchor requirement holds",
    )
    .status
    .success()
    .then_some(())
    .expect("the anchor Requirement must be valid");
    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args([
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "rule_trial",
            "--requirement-id",
            "req_anchor",
            "--statement",
            statement,
            "--status",
            "review",
            "--severity",
            "high",
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "the typed write must refuse the statement"
    );
    let refusal = error_json(&output);
    assert_named_rule_one_one_finding(&refusal["report"], "claim");
    assert_eq!(preflight["data"]["findings"], refusal["report"]["findings"]);
}

#[test]
fn requirement_preflight_matches_the_typed_write_for_the_trial_statement() {
    let (_scratch, repo, index_directory) = setup_dictionary();
    let statement = "Staff must attach at least one receipt before they submit an expense claim";
    let preflight = check_statement(&repo, &index_directory, statement);
    assert_named_rule_one_one_finding(&preflight["data"], "attach");
    assert_named_rule_one_one_finding(&preflight["data"], "claim");

    let output = create_requirement(&repo, &index_directory, "req_trial", statement);
    assert!(
        !output.status.success(),
        "the typed write must refuse the statement"
    );
    let refusal = error_json(&output);
    assert_named_rule_one_one_finding(&refusal["report"], "attach");
    assert_named_rule_one_one_finding(&refusal["report"], "claim");
    assert_eq!(preflight["data"]["findings"], refusal["report"]["findings"]);
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
#[verifies("rule_ste_dictionary_reference_resolution", examples)]
fn a_reference_with_a_loadable_index_rejects_an_unapproved_word() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    init(&repo);
    let dictionary = imported_dictionary();
    store_dictionary_index(dictionary, &index_directory).unwrap();
    write_reference(&repo, dictionary);

    let output = create_requirement(&repo, &index_directory, "req_gate", &statement());

    assert!(
        !output.status.success(),
        "the write must fail: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let error = error_json(&output);
    assert_eq!(error["kind"], "statement_invalid", "{error}");
    let findings = error["report"]["findings"]
        .as_array()
        .expect("the error lists findings");
    assert_eq!(findings.len(), 1, "findings: {findings:?}");
    assert_eq!(findings[0]["rule"], "1.1");
    assert_eq!(findings[0]["kind"], "violation");
    let span = &findings[0]["span"];
    let start = usize::try_from(span["start"].as_u64().unwrap()).unwrap();
    let end = usize::try_from(span["end"].as_u64().unwrap()).unwrap();
    assert_eq!(&statement()[start..end], UNAPPROVED_WORD);
}

#[test]
#[verifies("rule_ste_dictionary_reference_resolution", examples)]
fn a_project_without_a_reference_accepts_the_same_statement() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    init(&repo);
    store_dictionary_index(imported_dictionary(), &index_directory).unwrap();

    let output = create_requirement(&repo, &index_directory, "req_gate", &statement());

    assert!(
        output.status.success(),
        "the write must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_resolution", examples)]
fn a_reference_without_an_index_accepts_the_same_statement() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&index_directory).unwrap();
    init(&repo);
    write_reference(&repo, imported_dictionary());

    let output = create_requirement(&repo, &index_directory, "req_gate", &statement());

    assert!(
        output.status.success(),
        "a missing index must fall back to the data-free checks: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_resolution", examples)]
#[verifies("rule_ste_strict_committed_statement_gate", examples)]
fn strict_check_uses_the_imported_project_dictionary() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "--initial-branch", "main"]);
    git(&repo, &["config", "user.email", "test@example.test"]);
    git(&repo, &["config", "user.name", "Test"]);
    init(&repo);
    let dictionary = imported_dictionary();
    store_dictionary_index(dictionary, &index_directory).unwrap();
    create_requirement(&repo, &index_directory, "req_dictionary", &statement())
        .status
        .success()
        .then_some(())
        .expect("the statement is accepted before the project reference exists");
    write_reference(&repo, dictionary);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "initial candidate"]);

    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args([
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--strict",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let statements = report["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["category"] == "statements")
        .unwrap();
    assert_eq!(statements["status"], "findings");
    assert_eq!(statements["context"]["base_commit"], Value::Null);
    assert_eq!(statements["findings"][0]["detail"]["id"], "req_dictionary");
    assert_eq!(statements["findings"][0]["detail"]["rule"], "1.1");
}

fn git(repo: &std::path::Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
