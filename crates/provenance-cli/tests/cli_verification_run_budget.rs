//! The runs of a Rule through the real CLI `api` route: a run row inside the
//! byte budget of the resource reads leaves the evidence read of each Rule
//! readable, and a completion whose row passes that budget is refused.
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Output;

use crate::support::review_paths::provenance;

/// The bytes of one stored row that the page read and the member read of
/// the runs give.
const BUDGET: usize = 1_048_576 - 16_384;

fn api(repo: &Path, path: &str, input: Option<&Value>) -> Output {
    let root = repo.to_str().unwrap();
    let mut command = provenance();
    command
        .args(["api", path, "--repo", root, "--scope", "default"])
        .current_dir(repo);
    if let Some(input) = input {
        command
            .args(["--method", "post", "--input", "-"])
            .write_stdin(input.to_string());
    }
    command.output().unwrap()
}

fn data(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    value["data"].clone()
}

/// A repository with the Rules `rule_one` and `rule_two`, and the file that
/// a run cites.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    let root = repo.to_str().unwrap();
    provenance()
        .args(["init", "--path", root, "--scope", "default"])
        .args(["--path-prefix", "."])
        .current_dir(repo)
        .assert()
        .success();
    provenance()
        .args(["req_data", "create", "--type", "requirement", "--repo", root])
        .args(["--scope", "default", "--statement", "The system keeps the data"])
        .current_dir(repo)
        .assert()
        .success();
    for id in ["rule_one", "rule_two"] {
        let rule = json!({
            "id": id,
            "requirement_ids": ["req_data"],
            "resolution_ids": [],
            "statement": "The system shows the data",
            "status": "active",
            "severity": "high"
        });
        data(&api(repo, "rules", Some(&rule)));
    }
    std::fs::write(repo.join("check.rs"), "fn check() {}\n").unwrap();
    directory
}

/// A new run of `rule_one`, and the length of the error text that makes its
/// row `row` bytes long after a failed completion.
fn begin(repo: &Path, row: usize) -> (String, usize) {
    let start = json!({
        "rule": "rule_one", "key": "check", "method": "examples",
        "declared_by": "fixture", "file": "check.rs"
    });
    let run = data(&api(repo, "verification-runs/begin-verification", Some(&start)));
    let mut failed = run.clone();
    failed["status"] = json!("failed");
    failed["completed_at"] = run["started_at"].clone();
    failed["error"] = json!("");
    let base = serde_json::to_vec(&failed).unwrap().len();
    (run["id"].as_str().unwrap().to_owned(), row - base)
}

fn fail(repo: &Path, run: &str, error: usize) -> Output {
    api(
        repo,
        &format!("verification-runs/{run}/complete-verification"),
        Some(&json!({"status": "failed", "error": "x".repeat(error)})),
    )
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn the_evidence_read_of_each_rule_gives_a_run_row_inside_the_budget() {
    let directory = repository();
    let repo = directory.path();
    let (run, error) = begin(repo, BUDGET);

    let completed = data(&fail(repo, &run, error));
    let one = data(&api(repo, "rules/rule_one/evidence", None));
    let two = data(&api(repo, "rules/rule_two/evidence", None));

    assert_eq!(completed["error"].as_str().map(str::len), Some(error));
    assert_eq!(one["verification_runs"], json!([completed]));
    assert_eq!(two["verification_runs"], json!([]));
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn a_completion_with_a_row_over_the_budget_is_refused_and_the_run_stays_running() {
    let directory = repository();
    let repo = directory.path();
    let (run, error) = begin(repo, BUDGET + 1);

    let refused = fail(repo, &run, error);
    let member = data(&api(repo, &format!("verification-runs/{run}"), None));
    let evidence = data(&api(repo, "rules/rule_two/evidence", None));

    assert_eq!(refused.status.code(), Some(1));
    assert!(refused.stdout.is_empty());
    let stderr = String::from_utf8(refused.stderr).unwrap();
    let message = stderr.strip_prefix("Error: ").unwrap();
    let refusal: Value = serde_json::from_str(message).unwrap();
    assert_eq!(refusal["error"]["kind"], "record_too_large");
    assert_eq!(
        (&member["status"], &member["error"]),
        (&json!("running"), &Value::Null)
    );
    assert_eq!(evidence["verification_runs"], json!([]));
}
