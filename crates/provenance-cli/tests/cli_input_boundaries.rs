use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};

fn command(repo: &str, args: &[&str]) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    command.args(args).args(["--repo", repo, "--format", "json"]);
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
        .args(["init", "--path", repo, "--scope", "default", "--path-prefix", "."])
        .assert()
        .success();
    command(
        repo,
        &[
            "requirements", "create", "--id", "req_parent", "--statement",
            "The system records one parent.",
        ],
    )
    .assert()
    .success();
    directory
}

#[test]
fn discussion_list_refuses_repeated_scalar_flags() {
    let directory = fixture();
    let repo = directory.path().to_str().unwrap();
    command(
        repo,
        &["req_parent", "discussions", "--status", "active", "--status", "resolved"],
    )
    .assert()
    .code(2)
    .stderr(contains("--status"));
    let single = output(&mut command(repo, &["req_parent", "discussions", "--limit", "1"]));
    assert_eq!(single["limit"], 1);
}

#[test]
fn discussion_write_refuses_repeated_actor_without_creating_a_discussion() {
    let directory = fixture();
    let repo = directory.path().to_str().unwrap();
    command(
        repo,
        &["req_parent", "discuss", "--body", "hello", "--actor", "first", "--actor", "second"],
    )
    .assert()
    .code(2)
    .stderr(contains("--actor"));
    let list = output(&mut command(repo, &["req_parent", "discussions"]));
    assert_eq!(list["result"]["entries"], json!([]));
}

#[test]
fn free_string_array_items_preserve_json_looking_text() {
    let directory = fixture();
    let repo = directory.path().to_str().unwrap();
    let body = json!({
        "id": "contrib_literals",
        "target": {"artifact_type": "requirement", "artifact_id": "req_parent"},
        "participant_slot": "reviewer",
        "stance": "support",
        "strongest_finding": "The evidence contains literal examples.",
        "evidence_references": [],
        "material_claims": [],
        "objections": [],
        "challenges": [],
        "suggested_artifact_changes": [],
        "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct evidence"}
    });
    let created = output(
        command(
            repo,
            &["contributions", "create", "--stdin", "--risks", "[]", "--risks", "{}", "--open-questions", "null"],
        )
        .write_stdin(body.to_string()),
    );
    assert_eq!(created["data"]["risks"], json!(["[]", "{}"]));
    assert_eq!(created["data"]["open_questions"], json!(["null"]));
    let stored = output(&mut command(repo, &["contributions", "contrib_literals", "get"]));
    assert_eq!(stored["data"]["risks"], json!(["[]", "{}"]));
}
