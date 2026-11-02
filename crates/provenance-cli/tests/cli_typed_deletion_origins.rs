//! Typed-spec apply removes the origins that cite a deleted Discussion.
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
        let body = if args.first() == Some(&"api") {
            ["--input", "-"].as_slice()
        } else {
            ["--stdin"].as_slice()
        };
        command.args(body).write_stdin(input.to_string());
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn spec(sources: &Value) -> Value {
    json!({
        "schema_version": 2, "spec": "fx", "declared_by": "spec://fx",
        "sources": sources, "requirements": []
    })
}

/// The bytes of each file in the Rule record directory.
fn rule_files(repo: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let directory = repo.join(".provenance/state/scopes/default/rules");
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn start(repo: &Path, source: &str) -> Value {
    let started = provenance(
        repo,
        &[
            "api",
            &format!("sources/{source}/discussions"),
            "--method",
            "post",
        ],
        Some(json!({
            "actor":"reviewer", "declared_by":"spec://fx", "role":"user", "body":"A concern."
        })),
    );
    started["data"].clone()
}

fn origin(started: &Value) -> Value {
    json!({
        "discussion_id":started["discussion_id"], "thread_id":started["thread_id"],
        "message_id":started["root_message_id"]
    })
}

#[test]
#[verifies("rule_typed_deletion_clears_origins", examples)]
fn only_the_origins_that_cite_a_deleted_discussion_are_removed() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    let root = repo.to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["init", "--path", root, "--scope", "default"])
        .args(["--path-prefix", "."])
        .assert()
        .success();
    let kept_source = json!({"key":"kept", "name":"Kept", "kind":"policy"});
    let applied = provenance(
        repo,
        &["api", "authoring-changes", "--method", "post"],
        Some(spec(&json!([
            {"key":"src", "name":"Original", "kind":"policy"}, kept_source
        ]))),
    );
    let id = |key: &str| {
        applied["data"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|resource| resource["key"] == key)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let deleted = start(repo, &id("src"));
    let surviving = start(repo, &id("kept"));
    for (requirement, started) in [("req_from_talk", &deleted), ("req_from_kept", &surviving)] {
        let created = provenance(
            repo,
            &["api", "requirements", "--method", "post"],
            Some(json!({
                "actor":"reviewer", "id":requirement, "statement":"The host stops.",
                "status":"active", "depends_on":[], "supersedes":[], "origin":origin(started)
            })),
        );
        assert_eq!(created["data"]["origin_thread"], started["thread_id"]);
    }
    provenance(
        repo,
        &[
            "rule_from_kept",
            "create",
            "--type",
            "rule",
            "--requirement-id",
            "req_from_kept",
            "--statement",
            "The host stops.",
            "--origin-thread",
            surviving["thread_id"].as_str().unwrap(),
            "--origin-message",
            surviving["root_message_id"].as_str().unwrap(),
        ],
        None,
    );
    let rules_before = rule_files(repo);

    let applied = provenance(
        repo,
        &["api", "authoring-changes", "--method", "post"],
        Some(spec(&json!([kept_source]))),
    );

    assert_eq!(applied["data"]["deleted"], 1, "{applied}");
    let cleared = provenance(repo, &["api", "requirements/req_from_talk"], None);
    assert_eq!(cleared["data"]["origin_thread"], Value::Null, "{cleared}");
    assert_eq!(cleared["data"]["origin_message"], Value::Null, "{cleared}");
    for path in [
        ["api", "requirements/req_from_kept"],
        ["api", "rules/rule_from_kept"],
    ] {
        let kept = provenance(repo, &path, None);
        assert_eq!(
            kept["data"]["origin_thread"], surviving["thread_id"],
            "{kept}"
        );
        assert_eq!(
            kept["data"]["origin_message"], surviving["root_message_id"],
            "{kept}"
        );
    }
    // No Rule cites the deleted Discussion, so the Rule file is not written.
    assert_eq!(rule_files(repo), rules_before);
    provenance(repo, &["check"], None);
}
