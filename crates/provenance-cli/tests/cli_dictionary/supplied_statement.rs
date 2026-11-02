use crate::support::{
    create_requirement, error_json, imported_dictionary, init, provenance, write_reference,
    write_reference_with_technical_names, UNAPPROVED_WORD,
};
use provenance_macros::verifies;
use provenance_ste100::store_dictionary_index;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Output;

const WARNING: &str = "ASD-STE100 Rule 1.1 did not run";

/// A statement that breaks Rule 8.1 without a dictionary: it has a semicolon.
const SEMICOLON_STATEMENT: &str = "The pump stops; the valve opens.";

fn unapproved_statement() -> String {
    format!("The {UNAPPROVED_WORD} item stops.")
}

/// A repository with a committed reference. The index is stored only on request.
fn setup(store_index: bool) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&index_directory).unwrap();
    init(&repo);
    let dictionary = imported_dictionary();
    if store_index {
        store_dictionary_index(dictionary, &index_directory).unwrap();
    }
    write_reference(&repo, dictionary);
    (scratch, repo, index_directory)
}

fn check(repo: &Path, index_directory: &Path, statement: &str, extra: &[&str]) -> Output {
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
        .args([
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--statement",
            statement,
        ])
        .args(extra)
        .output()
        .unwrap()
}

fn check_report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be the check report: {error}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn statements_category(report: &Value) -> &Value {
    let categories = report["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1, "{report}");
    assert_eq!(categories[0]["category"], "statements", "{report}");
    &categories[0]
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement", examples)]
fn an_unapproved_word_gives_a_rule_one_one_finding_with_the_ste100_detail() {
    let (_scratch, repo, index_directory) = setup(true);
    let statement = unapproved_statement();

    let output = check(&repo, &index_directory, &statement, &["--format", "json"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = check_report(&output);
    let statements = statements_category(&report);
    assert_eq!(statements["status"], "findings", "{report}");
    assert_eq!(statements["context"], Value::Null, "{report}");
    let findings = statements["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{report}");
    let detail = &findings[0]["detail"];
    assert_eq!(detail["rule"], "1.1", "{report}");
    assert_eq!(detail["kind"], "violation", "{report}");
    assert_eq!(detail["message"], findings[0]["message"], "{report}");
    let start = usize::try_from(detail["span"]["start"].as_u64().unwrap()).unwrap();
    let end = usize::try_from(detail["span"]["end"].as_u64().unwrap()).unwrap();
    assert_eq!(&statement[start..end], UNAPPROVED_WORD);
    assert!(
        detail
            .as_object()
            .unwrap()
            .keys()
            .all(|key| { ["rule", "kind", "span", "message"].contains(&key.as_str()) }),
        "the detail is the ste100 finding alone: {detail}"
    );
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement", examples)]
fn a_listed_technical_name_and_a_clean_statement_pass() {
    let (_scratch, repo, index_directory) = setup(true);
    write_reference_with_technical_names(&repo, imported_dictionary(), &[UNAPPROVED_WORD]);

    for statement in [unapproved_statement(), "The pump stops.".to_owned()] {
        let output = check(&repo, &index_directory, &statement, &["--format", "json"]);

        assert!(output.status.success(), "{statement}");
        let report = check_report(&output);
        let statements = statements_category(&report);
        assert_eq!(statements["status"], "passed", "{statement}: {report}");
        assert_eq!(statements["findings"], Value::Array(Vec::new()), "{report}");
    }
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement", examples)]
fn findings_exit_zero_without_strict_and_one_with_strict() {
    let (_scratch, repo, index_directory) = setup(true);
    let statement = unapproved_statement();

    let readable = check(&repo, &index_directory, &statement, &[]);
    let strict = check(&repo, &index_directory, &statement, &["--strict"]);

    assert!(readable.status.success());
    let stdout = String::from_utf8_lossy(&readable.stdout);
    assert!(stdout.starts_with("statements: findings\n  - "), "{stdout}");
    assert!(stdout.contains(UNAPPROVED_WORD), "{stdout}");
    assert_eq!(strict.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&strict.stdout),
        stdout,
        "strict mode prints the same report"
    );
    assert!(
        String::from_utf8_lossy(&strict.stderr).contains(UNAPPROVED_WORD),
        "the strict error names the finding"
    );
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement_matches_write", examples)]
fn a_missing_index_warns_first_and_is_unavailable_in_strict_mode() {
    let (_scratch, repo, index_directory) = setup(false);

    let warned = check(
        &repo,
        &index_directory,
        SEMICOLON_STATEMENT,
        &["--format", "json"],
    );
    let strict = check(
        &repo,
        &index_directory,
        SEMICOLON_STATEMENT,
        &["--strict", "--format", "json"],
    );

    assert!(warned.status.success());
    let report = check_report(&warned);
    let findings = statements_category(&report)["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2, "{report}");
    assert!(
        findings[0]["message"]
            .as_str()
            .unwrap()
            .starts_with(WARNING),
        "{report}"
    );
    assert_eq!(findings[0]["detail"], Value::Null, "{report}");
    assert_eq!(findings[1]["detail"]["rule"], "8.1", "{report}");

    assert_eq!(strict.status.code(), Some(1));
    let report = check_report(&strict);
    let statements = statements_category(&report);
    assert_eq!(statements["status"], "unavailable", "{report}");
    assert!(
        statements["unavailable_reason"]
            .as_str()
            .unwrap()
            .contains("provenance dictionary import"),
        "{report}"
    );
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement", examples)]
fn an_uninitialized_repository_is_unavailable() {
    let scratch = tempfile::tempdir().unwrap();

    let output = check(
        scratch.path(),
        scratch.path(),
        "The pump stops.",
        &["--format", "json"],
    );

    assert_eq!(output.status.code(), Some(1));
    let report = check_report(&output);
    let statements = statements_category(&report);
    assert_eq!(statements["status"], "unavailable", "{report}");
    assert!(
        statements["unavailable_reason"]
            .as_str()
            .unwrap()
            .contains("provenance init"),
        "{report}"
    );
    assert!(!scratch.path().join(".provenance").exists());
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement", examples)]
fn a_supplied_statement_replaces_the_changed_committed_statements() {
    let (_scratch, repo, index_directory) = setup(true);
    git(&repo, &["init", "--initial-branch", "main"]);
    git(&repo, &["config", "user.email", "test@example.test"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "initial"]);
    write_reference_with_technical_names(&repo, imported_dictionary(), &[UNAPPROVED_WORD]);
    create_requirement(
        &repo,
        &index_directory,
        "req_changed",
        &unapproved_statement(),
    )
    .status
    .success()
    .then_some(())
    .expect("the listed name lets the write pass");
    write_reference(&repo, imported_dictionary());

    let committed = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args(["check", "--statements", "--repo", repo.to_str().unwrap()])
        .args(["--format", "json"])
        .output()
        .unwrap();
    let supplied = check(
        &repo,
        &index_directory,
        "The pump stops.",
        &["--format", "json"],
    );

    let report = check_report(&committed);
    assert_eq!(
        statements_category(&report)["findings"][0]["detail"]["id"],
        "req_changed",
        "the committed check sees the changed record: {report}"
    );
    let report = check_report(&supplied);
    assert_eq!(statements_category(&report)["status"], "passed", "{report}");
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement", examples)]
fn a_supplied_statement_refuses_a_base_commit() {
    let (_scratch, repo, index_directory) = setup(true);

    let output = check(
        &repo,
        &index_directory,
        "The pump stops.",
        &["--strict", "--base", "HEAD"],
    );

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--base") && stderr.contains("--statement"),
        "{stderr}"
    );
}

#[test]
#[verifies("rule_porcelain_check_supplied_statement_matches_write", examples)]
fn the_check_findings_equal_the_rule_write_refusal() {
    let (_scratch, repo, index_directory) = setup(true);
    let statement = format!("The {UNAPPROVED_WORD} item stops; the claim item stops.");
    create_requirement(&repo, &index_directory, "req_anchor", "The pump stops.")
        .status
        .success()
        .then_some(())
        .expect("the anchor Requirement must be valid");

    let checked = check(&repo, &index_directory, &statement, &["--format", "json"]);
    let write = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .args(["rule_trial", "create", "--type", "rule"])
        .args(["--repo", repo.to_str().unwrap(), "--scope", "default"])
        .args(["--requirement-id", "req_anchor", "--statement", &statement])
        .args([
            "--status",
            "review",
            "--severity",
            "high",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(checked.status.success());
    let report = check_report(&checked);
    let details = statements_category(&report)["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["detail"].clone())
        .collect::<Vec<_>>();
    assert_eq!(details.len(), 3, "{report}");
    assert!(
        !write.status.success(),
        "the write must refuse the statement"
    );
    let refusal = error_json(&write);
    assert_eq!(refusal["kind"], "statement_invalid", "{refusal}");
    assert_eq!(refusal["report"]["findings"], Value::Array(details));
}

fn git(repo: &Path, arguments: &[&str]) {
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
