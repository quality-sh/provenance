use crate::support::{
    create_requirement, imported_dictionary, init, provenance, write_reference, UNAPPROVED_NOUN,
    UNAPPROVED_VERB,
};
use provenance_macros::verifies;
use provenance_ste100::store_dictionary_index;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn setup(repo_is_git: bool) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    std::fs::create_dir_all(&repo).unwrap();
    if repo_is_git {
        git(&repo, &["init", "--initial-branch", "main"]);
        git(&repo, &["config", "user.email", "test@example.test"]);
        git(&repo, &["config", "user.name", "Test"]);
    }
    init(&repo);
    store_dictionary_index(imported_dictionary(), &index_directory).unwrap();
    (scratch, repo, index_directory)
}

/// Runs the statement preflight and returns the text of each Rule 1.1 finding.
fn rule_one_one_words(repo: &Path, index_directory: &Path, statement: &str) -> Vec<String> {
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
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    report["data"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == "1.1")
        .map(|finding| {
            let start = usize::try_from(finding["span"]["start"].as_u64().unwrap()).unwrap();
            let end = usize::try_from(finding["span"]["end"].as_u64().unwrap()).unwrap();
            statement[start..end].to_owned()
        })
        .collect()
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
fn the_preflight_reports_each_inflection_of_an_unapproved_word() {
    let (_scratch, repo, index_directory) = setup(false);
    write_reference(&repo, imported_dictionary());
    let stem = UNAPPROVED_VERB.strip_suffix('e').unwrap();
    for (statement, word) in [
        (
            format!("The registry {UNAPPROVED_VERB}s the command."),
            format!("{UNAPPROVED_VERB}s"),
        ),
        (
            format!("The registry {UNAPPROVED_VERB}d the command."),
            format!("{UNAPPROVED_VERB}d"),
        ),
        (
            format!("The registry is {stem}ing the command."),
            format!("{stem}ing"),
        ),
        (
            format!("The {UNAPPROVED_NOUN}s stay here."),
            format!("{UNAPPROVED_NOUN}s"),
        ),
    ] {
        assert_eq!(
            rule_one_one_words(&repo, &index_directory, &statement),
            [word],
            "statement: {statement}"
        );
    }
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
#[verifies("rule_ste_strict_committed_statement_gate", examples)]
fn strict_check_reports_an_inflected_unapproved_verb_in_a_committed_requirement() {
    let (_scratch, repo, index_directory) = setup(true);
    let statement = format!("The registry {UNAPPROVED_VERB}s the command.");
    create_requirement(&repo, &index_directory, "req_inflected", &statement)
        .status
        .success()
        .then_some(())
        .expect("the statement is accepted before the project reference exists");
    write_reference(&repo, imported_dictionary());
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
    let finding = &statements["findings"][0]["detail"];
    assert_eq!(finding["id"], "req_inflected");
    assert_eq!(finding["rule"], "1.1");
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
