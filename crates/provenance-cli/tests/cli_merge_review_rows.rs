//! The merge driver merges the review rows of a requirements directory by id.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;

const REVIEW_SHARD: &str = ".provenance/state/scopes/default/requirements/review.jsonl";

fn review_row(id: &str, after: &str) -> String {
    format!(
        "{{\"scope_id\":\"default\",\"id\":\"{id}\",\"rule_id\":\"rule_a\",\
         \"requirement_id\":\"req_a\",\"field\":\"statement\",\
         \"before\":\"The old text.\",\"after\":\"{after}\",\"changed_at\":1}}\n"
    )
}

fn merge(directory: &Path, base: &str, ours: &str, theirs: &str) -> std::process::Output {
    let path = |name: &str| directory.join(name);
    std::fs::write(path("base.jsonl"), base).unwrap();
    std::fs::write(path("ours.jsonl"), ours).unwrap();
    std::fs::write(path("theirs.jsonl"), theirs).unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .arg("merge-jsonl")
        .arg(path("base.jsonl"))
        .arg(path("ours.jsonl"))
        .arg(path("theirs.jsonl"))
        .arg("--output")
        .arg(path("merged.jsonl"))
        .args(["--path", REVIEW_SHARD, "--format", "json"])
        .output()
        .unwrap()
}

#[test]
#[verifies("rule_record_merge", examples)]
fn different_added_review_rows_merge_to_both_rows() {
    let directory = tempfile::tempdir().unwrap();
    let base = review_row("requirement_review_base", "The base text.");
    let ours = format!("{base}{}", review_row("requirement_review_ours", "Ours."));
    let theirs = format!(
        "{base}{}",
        review_row("requirement_review_theirs", "Theirs.")
    );

    let output = merge(directory.path(), &base, &ours, &theirs);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let merged = std::fs::read_to_string(directory.path().join("merged.jsonl")).unwrap();
    let ids: Vec<String> = merged
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["id"].to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "\"requirement_review_base\"",
            "\"requirement_review_ours\"",
            "\"requirement_review_theirs\"",
        ],
        "{merged}"
    );
}

#[test]
#[verifies("rule_record_merge", examples)]
fn one_review_row_changed_on_both_sides_reports_a_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let base = review_row("requirement_review_a", "The base text.");
    let ours = review_row("requirement_review_a", "Ours.");
    let theirs = review_row("requirement_review_a", "Theirs.");

    let output = merge(directory.path(), &base, &ours, &theirs);

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{error}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(report["status"], "conflicted");
    assert_eq!(report["conflicts"][0]["kind"], "divergent_edit");
    assert_eq!(report["conflicts"][0]["record_id"], "requirement_review_a");
    assert_eq!(report["partial"][0]["after"], "Ours.");
}
