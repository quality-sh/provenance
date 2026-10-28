use assert_cmd::Command;
use serde_json::json;
use std::path::Path;

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
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_anchor",
            "--statement",
            "The anchor requirement holds",
        ])
        .assert()
        .success();
}

pub fn create_rule(repo: &Path, id: &str, status: &str) {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args([
        "rules",
        "create",
        "--repo",
        repo.to_str().unwrap(),
        "--scope",
        "default",
        "--stdin",
    ]);
    let archived = (status == "archived")
        .then(|| json!({"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}));
    command
        .write_stdin(
            json!({
                "id": id,
                "requirement_ids": ["req_anchor"],
                "resolution_ids": [],
                "statement": "Payroll follows the current policy",
                "status": status,
                "severity": "high",
                "archived_in_commit": archived
            })
            .to_string(),
        )
        .assert()
        .success();
}
