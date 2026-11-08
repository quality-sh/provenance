//! The archive commit flag accepts a plain commit hash on the CLI.
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn run(repo: &str, args: &[&str]) -> Value {
    let result = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo, "--format", "json"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

fn repository() -> tempfile::TempDir {
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
    run(
        repo,
        &[
            "req_plain",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system is safe.",
        ],
    );
    run(
        repo,
        &[
            "rule_plain",
            "create",
            "--type",
            "rule",
            "--statement",
            "The system is safe.",
            "--requirement-ids",
            "req_plain",
        ],
    );
    directory
}

fn archive(repo: &str, id: &str, commit_flags: &[&str]) -> Value {
    let current = run(repo, &[id, "get"]);
    let etag = current["edit"]["etag"].as_str().unwrap().to_owned();
    let mut args = vec![id, "update", "--if-match", &etag, "--status", "archived"];
    args.extend_from_slice(commit_flags);
    run(repo, &args);
    run(repo, &[id, "get"])["record"]["value"].clone()
}

#[test]
#[verifies("rule_cli_archive_commit_plain_flag", examples)]
fn plain_archive_commit_flag_archives_a_rule_and_a_requirement() {
    let directory = repository();
    let repo = directory.path().to_str().unwrap();
    for id in ["rule_plain", "req_plain"] {
        let archived = archive(repo, id, &["--archived-in-commit", COMMIT]);
        assert_eq!(archived["status"], "archived", "{id}: {archived}");
        assert_eq!(
            archived["archived_in_commit"],
            serde_json::json!({ "commit": COMMIT }),
            "{id}: {archived}"
        );
    }
}

#[test]
#[verifies("rule_cli_archive_commit_plain_flag", examples)]
fn plain_and_json_archive_commit_flags_store_the_same_commit() {
    let plain = repository();
    let json = repository();
    let from_plain = archive(
        plain.path().to_str().unwrap(),
        "rule_plain",
        &["--archived-in-commit", COMMIT],
    );
    let object = format!(r#"{{"commit":"{COMMIT}"}}"#);
    let from_json = archive(
        json.path().to_str().unwrap(),
        "rule_plain",
        &["--archived-in-commit-json", &object],
    );
    assert_eq!(
        from_plain["archived_in_commit"],
        from_json["archived_in_commit"]
    );
}

#[test]
fn plain_archive_commit_flag_refuses_a_value_that_is_not_a_commit_hash() {
    let directory = repository();
    let repo = directory.path().to_str().unwrap();
    let current = run(repo, &["rule_plain", "get"]);
    let etag = current["edit"]["etag"].as_str().unwrap().to_owned();
    let result = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["rule_plain", "update", "--if-match", &etag])
        .args(["--status", "archived", "--archived-in-commit", "main"])
        .args(["--repo", repo])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("invalid value for --archived-in-commit"),
        "{stderr}"
    );
}
