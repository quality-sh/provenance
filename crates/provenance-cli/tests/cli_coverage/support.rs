use assert_cmd::Command;
use serde_json::json;
use std::path::Path;

use crate::support::record_review::{activate_rule, archive_rule};

pub fn init_repo(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "req_anchor",
            "create",
            "--type",
            "requirement",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--statement",
            "The anchor requirement holds",
        ])
        .assert()
        .success();
}

/// Creates a Rule with `status`. An active or archived Rule is created as a
/// draft, and the reviewer accepts its status request.
pub fn create_rule(repo: &Path, id: &str, status: &str) {
    let requested = matches!(status, "active" | "archived");
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args([
        "api",
        "rules",
        "--method",
        "post",
        "--input",
        "-",
        "--repo",
        repo.to_str().unwrap(),
        "--scope",
        "default",
    ]);
    command
        .write_stdin(
            json!({
                "id": id,
                "requirement_ids": ["req_anchor"],
                "resolution_ids": [],
                "statement": "Payroll follows the current policy",
                "status": if requested { "draft" } else { status },
                "severity": "high",
                "archived_in_commit": null
            })
            .to_string(),
        )
        .assert()
        .success();
    let repo = repo.to_str().unwrap();
    match status {
        "active" => activate_rule(repo, id),
        "archived" => archive_rule(repo, id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        _ => {}
    }
}
