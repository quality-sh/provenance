use assert_cmd::Command;
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
        "--id",
        id,
        "--requirement-id",
        "req_anchor",
        "--statement",
        "Payroll follows the current policy",
        "--status",
        status,
        "--severity",
        "high",
    ]);
    if status == "archived" {
        command.args([
            "--archived-in-commit",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ]);
    }
    command.assert().success();
}
