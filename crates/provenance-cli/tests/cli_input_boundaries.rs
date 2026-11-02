use assert_cmd::Command;
use predicates::str::contains;
use provenance_macros::verifies;
use serde_json::{json, Value};

fn command(repo: &str, args: &[&str]) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    command
        .args(args)
        .args(["--repo", repo, "--format", "json"]);
    command
}

fn output(command: &mut Command) -> Value {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

fn fixture() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    command(
        repo,
        &[
            "req_parent",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system records one parent.",
        ],
    )
    .assert()
    .success();
    directory
}

#[test]
#[verifies("rule_cli_repeated_scalar_options", examples)]
fn discussion_list_refuses_repeated_scalar_flags() {
    let directory = fixture();
    let repo = directory.path().to_str().unwrap();
    command(
        repo,
        &[
            "req_parent",
            "discussions",
            "--status",
            "active",
            "--status",
            "resolved",
        ],
    )
    .assert()
    .code(2)
    .stderr(contains("--status is supplied more than once"));
    let single = output(&mut command(
        repo,
        &["req_parent", "discussions", "--status", "active"],
    ));
    assert_eq!(single["result"]["entries"], json!([]));
}

#[test]
#[verifies("rule_cli_repeated_scalar_options", examples)]
fn discussion_write_refuses_repeated_actor_without_creating_a_discussion() {
    let directory = fixture();
    let repo = directory.path().to_str().unwrap();
    command(
        repo,
        &[
            "req_parent",
            "discuss",
            "--body",
            "hello",
            "--actor",
            "first",
            "--actor",
            "second",
        ],
    )
    .assert()
    .code(2)
    .stderr(contains("--actor is supplied more than once"));
    let list = output(&mut command(repo, &["req_parent", "discussions"]));
    assert_eq!(list["result"]["entries"], json!([]));
}
