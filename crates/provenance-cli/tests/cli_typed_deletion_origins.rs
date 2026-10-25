//! Typed-spec apply removes the origin that cites a deleted Discussion.
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

fn provenance(repo: &Path, args: &[&str], input: Option<Value>) -> Value {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    command
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--format", "json"]);
    if let Some(input) = input {
        command.arg("--stdin").write_stdin(input.to_string());
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn spec(sources: Value) -> Value {
    json!({
        "schema_version": 2, "spec": "fx", "declared_by": "spec://fx",
        "sources": sources, "requirements": []
    })
}

#[test]
#[verifies("rule_typed_deletion_deletes_discussions", examples)]
fn a_requirement_from_a_deleted_discussion_loses_its_origin() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    let root = repo.to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["init", "--path", root, "--scope", "default"])
        .args(["--path-prefix", "."])
        .assert()
        .success();
    let applied = provenance(
        repo,
        &["authoring-changes", "create"],
        Some(spec(
            json!([{"key":"src", "name":"Original", "kind":"policy"}]),
        )),
    );
    let source = applied["data"]["resources"][0]["id"].as_str().unwrap();
    let started = provenance(
        repo,
        &["sources", source, "discussions", "create"],
        Some(json!({
            "actor":"reviewer", "declared_by":"spec://fx", "role":"user", "body":"A concern."
        })),
    );
    let started = &started["data"];
    let created = provenance(
        repo,
        &["requirements", "create"],
        Some(json!({
            "actor":"reviewer", "id":"req_from_talk", "statement":"The host stops.",
            "status":"active", "depends_on":[], "supersedes":[],
            "origin":{
                "discussion_id":started["discussion_id"], "thread_id":started["thread_id"],
                "message_id":started["root_message_id"]
            }
        })),
    );
    assert_eq!(created["data"]["origin_thread"], started["thread_id"]);

    let applied = provenance(
        repo,
        &["authoring-changes", "create"],
        Some(spec(json!([]))),
    );

    assert_eq!(applied["data"]["deleted"], 1, "{applied}");
    let kept = provenance(repo, &["requirements", "req_from_talk", "get"], None);
    assert_eq!(kept["data"]["origin_thread"], Value::Null, "{kept}");
    assert_eq!(kept["data"]["origin_message"], Value::Null, "{kept}");
    provenance(repo, &["check"], None);
}
