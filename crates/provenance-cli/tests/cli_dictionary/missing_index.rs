use crate::support::{create_requirement, imported_dictionary, init, provenance, write_reference};
use provenance_macros::verifies;
use serde_json::Value;
use std::path::{Path, PathBuf};

const WARNING: &str = "ASD-STE100 Rule 1.1 did not run";

/// A statement that breaks Rule 8.1, which needs no dictionary: it has a semicolon.
const REFUSED_STATEMENT: &str = "The pump stops; the valve opens.";

/// A repository with a committed reference and an empty machine index directory.
fn setup(with_reference: bool) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("empty-index");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&index_directory).unwrap();
    init(&repo);
    if with_reference {
        write_reference(&repo, imported_dictionary());
    }
    (scratch, repo, index_directory)
}

fn statement_check(repo: &Path, index_directory: &Path) -> std::process::Output {
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
        .args([
            "check",
            "--statements",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_write_without_the_index_passes_with_a_warning_that_rule_1_1_did_not_run() {
    let (_scratch, repo, index_directory) = setup(true);

    let output = create_requirement(&repo, &index_directory, "req_warned", "The pump stops.");

    assert!(
        output.status.success(),
        "the missing index must not refuse the write: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("warning: {WARNING}")), "{stderr}");
    assert!(
        stderr.contains(&index_directory.display().to_string()),
        "the warning must name the index directory: {stderr}"
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_statement_check_without_the_index_passes_with_a_warning_that_rule_1_1_did_not_run() {
    let (_scratch, repo, index_directory) = setup(true);

    let output = statement_check(&repo, &index_directory);

    assert!(
        output.status.success(),
        "the missing index must not fail the check: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let statements = &report["categories"][0];
    assert_eq!(statements["category"], "statements");
    let messages: Vec<&str> = statements["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["message"].as_str().unwrap())
        .collect();
    assert!(
        messages.iter().any(|message| message.starts_with(WARNING)),
        "the check must say that Rule 1.1 did not run: {statements}"
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_project_without_a_reference_gets_no_warning() {
    let (_scratch, repo, index_directory) = setup(false);

    let write = create_requirement(&repo, &index_directory, "req_plain", "The pump stops.");
    let check = statement_check(&repo, &index_directory);

    assert!(write.status.success());
    assert!(!String::from_utf8_lossy(&write.stderr).contains(WARNING));
    assert!(check.status.success());
    assert!(!String::from_utf8_lossy(&check.stdout).contains(WARNING));
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_refused_json_write_without_the_index_says_in_the_refusal_that_rule_1_1_did_not_run() {
    let (_scratch, repo, index_directory) = setup(true);

    let output = create_requirement(&repo, &index_directory, "req_refused", REFUSED_STATEMENT);

    assert!(!output.status.success(), "the write must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let refusal: Value = serde_json::from_str(stderr.trim_end()).unwrap_or_else(|error| {
        panic!("standard error must be one JSON object: {error}: {stderr}")
    });
    assert_eq!(refusal["error"]["kind"], "statement_invalid", "{refusal}");
    let warnings = refusal["meta"]["warnings"]
        .as_array()
        .unwrap_or_else(|| panic!("the refusal must carry the warning: {refusal}"));
    assert!(
        warnings
            .iter()
            .any(|warning| warning.as_str().unwrap().starts_with(WARNING)),
        "{refusal}"
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_refused_text_write_without_the_index_prints_the_warning() {
    let (_scratch, repo, index_directory) = setup(true);

    for arguments in [
        vec!["requirements", "create", "--id", "req_text"],
        vec!["req_target", "create", "--type", "requirement"],
    ] {
        let output = provenance()
            .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
            .args(arguments)
            .args([
                "--repo",
                repo.to_str().unwrap(),
                "--scope",
                "default",
                "--statement",
                REFUSED_STATEMENT,
            ])
            .output()
            .unwrap();

        assert!(!output.status.success(), "the write must fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("warning: {WARNING}")), "{stderr}");
    }
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_preflight_without_the_index_gives_a_json_report_and_the_warning() {
    let (_scratch, repo, index_directory) = setup(true);

    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .write_stdin(serde_json::json!({"statement": REFUSED_STATEMENT}).to_string())
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

    assert!(output.status.success(), "the preflight must succeed");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        !report["data"]["findings"].as_array().unwrap().is_empty(),
        "the preflight must report the semicolon: {report}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("warning: {WARNING}")), "{stderr}");
}
