use crate::support::{create_requirement, imported_dictionary, init, provenance, write_reference};
use provenance_macros::verifies;
use serde_json::Value;
use std::path::{Path, PathBuf};

const WARNING: &str = "ASD-STE100 Rule 1.1 did not run";

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
