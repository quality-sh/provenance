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

/// Runs a repeated option in text mode and with `--format json`. Text mode
/// gives the usage text with exit code 2; JSON mode gives one `invalid_input`
/// object with exit code 1. Each names the option.
fn refuses_repeated(repo: &str, args: &[&str], message: &str) {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo])
        .assert()
        .code(2)
        .stderr(contains(message));
    let refused = command(repo, args).assert().code(1);
    let stderr = String::from_utf8(refused.get_output().stderr.clone()).unwrap();
    let refusal: Value = serde_json::from_str(&stderr).unwrap();
    assert_eq!(refusal["error"]["kind"], "invalid_input", "{refusal}");
    let text = refusal["error"]["message"].as_str().unwrap();
    assert!(text.contains(message), "{refusal}");
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
    refuses_repeated(
        repo,
        &[
            "req_parent",
            "discussions",
            "--status",
            "active",
            "--status",
            "resolved",
        ],
        "--status is supplied more than once",
    );
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
    refuses_repeated(
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
        "--actor is supplied more than once",
    );
    let list = output(&mut command(repo, &["req_parent", "discussions"]));
    assert_eq!(list["result"]["entries"], json!([]));
}

/// A query parameter flag with a closed set lists the set, in schema order, as
/// a body flag does.
#[verifies("rule_cli_closed_value_refusal_lists_values", examples)]
#[test]
fn invalid_discussion_status_lists_the_accepted_values() {
    let directory = fixture();
    let repo = directory.path().to_str().unwrap();
    let refused = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["discussions", "--status", "bogus", "--repo", repo])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        String::from_utf8_lossy(&refused.stderr).trim_end(),
        "error: invalid value for --status; accepted values: active, resolved, all"
    );
}
