//! A store failure of no known kind keeps the code `write_failed` or
//! `read_failed`, and the refusal object gives the cause chain in `cause`.
//! A path below the repository in the cause is relative to the repository.

use provenance_macros::verifies;
use serde_json::Value;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, provenance};

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

/// The start of the cause. The rest is the text of the operating system.
const CAUSE: &str = "failed to persist temporary file: ";

/// A repository whose requirement shard is a directory, so that a write of
/// the family fails with an I/O error of no known kind.
fn shard_is_a_directory() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let path = directory.path().join(REQUIREMENTS);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    (directory, repo)
}

fn create(repo: &str, json: bool) -> std::process::Output {
    let mut command = provenance();
    command.args([
        "req_b",
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--statement",
        "The system records two topics.",
    ]);
    if json {
        command.args(["--format", "json"]);
    }
    command.output().unwrap()
}

/// The refusal object on standard error of a command that exits 1, with or
/// without the `Error: ` prefix.
fn refusal(output: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// Asserts the code and the start of the cause, and that no spelling of the
/// temporary directory is on standard error or in the cause.
fn assert_cause(directory: &tempfile::TempDir, output: &std::process::Output, kind: &str) {
    let refusal = refusal(output);
    assert_eq!(refusal["error"]["kind"], kind, "{refusal}");
    let cause = refusal["error"]["cause"].as_str().unwrap_or_default();
    assert!(cause.starts_with(CAUSE), "{refusal}");
    let text = String::from_utf8_lossy(&output.stderr);
    let canonical = std::fs::canonicalize(directory.path()).unwrap();
    for root in [directory.path(), canonical.as_path()] {
        let root = root.to_string_lossy();
        let plain = root.strip_prefix(r"\\?\").unwrap_or(&root);
        for spelling in [plain.to_owned(), plain.replace('\\', "/")] {
            assert!(
                !text.contains(&spelling) && !cause.contains(&spelling),
                "{spelling} in {text}"
            );
        }
    }
}

#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn text_mode_prints_the_cause_of_a_write_failure_of_no_known_kind() {
    let (directory, repo) = shard_is_a_directory();
    let output = create(&repo, false);
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(text.starts_with("Error: "), "{text}");
    assert_eq!(text.lines().count(), 1, "{text}");
    assert_cause(&directory, &output, "write_failed");
}

#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn json_mode_gives_the_cause_of_a_write_failure_of_no_known_kind() {
    let (directory, repo) = shard_is_a_directory();
    let output = create(&repo, true);
    assert_cause(&directory, &output, "write_failed");
    let refusal = refusal(&output);
    assert_eq!(refusal["meta"], serde_json::json!({}), "{refusal}");
}

/// A requirement shard that is not UTF-8 is a fault of no known kind. Its
/// cause names the shard by its path relative to the repository.
#[test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
fn the_cause_of_a_write_failure_gives_the_shard_path_relative_to_the_repository() {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    std::fs::write(directory.path().join(REQUIREMENTS), b"\xff\n").unwrap();
    let refusal = refusal(&create(&repo, true));
    assert_eq!(
        refusal["error"],
        serde_json::json!({"kind":"write_failed",
            "cause":format!("read {REQUIREMENTS}: the file is not UTF-8: invalid data")}),
        "{refusal}"
    );
}
