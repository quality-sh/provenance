//! Action help on an existing record lists the fields that the action
//! accepts for that record type, and a Discussion action without a required
//! option is a usage error.

use fs2::FileExt as _;
use predicates::prelude::*;
use provenance_macros::verifies;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, init, json_output, provenance};

fn init_with_requirement() -> (tempfile::TempDir, String) {
    let (directory, repo) = init();
    create_requirement(&repo, "req_a");
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

/// Every file under the directory with its bytes.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.insert(path.clone(), Vec::new());
                pending.push(path);
            } else {
                files.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

#[test]
#[verifies("rule_create_help_accepted_fields", examples)]
fn update_help_lists_only_the_fields_of_the_existing_record_type() {
    let (_directory, repo) = init_with_requirement();

    let text = help(&repo, &["req_a", "update"]);
    assert!(text.contains("--statement"), "{text}");
    assert!(text.contains("--if-match"), "{text}");
    for flag in ["--url", "--expected-etag", "--position", "--made-by"] {
        assert!(!text.contains(flag), "{flag} in\n{text}");
    }
}

#[test]
#[verifies("rule_create_help_accepted_fields", examples)]
fn header_bound_body_fields_stay_out_of_help_and_are_usage_errors() {
    let (_directory, repo) = init_with_requirement();

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
        .code(2)
        .stderr(predicate::str::contains(
            "unexpected argument '--expected-etag'",
        ));
}

#[test]
#[verifies("rule_create_help_accepted_fields", examples)]
fn reply_help_marks_expected_version_required_and_its_absence_is_a_usage_error() {
    let (_directory, repo) = init_with_requirement();
    let started = json_output(provenance().args([
        "req_a", "discuss", "--repo", &repo, "--body", "hello", "--format", "json",
    ]));
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

#[test]
#[verifies("rule_record_help_takes_no_lock", examples)]
fn help_returns_at_once_while_another_process_holds_the_publication_lock() {
    let (directory, repo) = init_with_requirement();
    let locks = directory.path().join(".provenance/cache/locks");
    std::fs::create_dir_all(&locks).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(locks.join("repository.publication.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let before = tree(directory.path());

    provenance()
        .args(["req_a", "update", "--repo", &repo, "--help"])
        .timeout(Duration::from_secs(30))
        .assert()
        .success()
        .stdout(predicate::str::contains("--statement"))
        .stdout(predicate::str::contains("--url").not());

    assert_eq!(tree(directory.path()), before);
    fs2::FileExt::unlock(&lock).unwrap();
}
