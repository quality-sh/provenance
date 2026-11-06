//! Typed-spec apply deletes a Discussion whose Thread has Messages in an
//! earlier month shard.
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

const THREAD: &str = "thread_source_source_legacy";

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

fn git(repo: &Path, args: &[&str]) {
    let output = provenance_store::git_process::git()
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn spec(sources: &Value, adopt: &Value) -> Value {
    json!({
        "schema_version": 2, "spec": "fx", "declared_by": "spec://fx",
        "adopt_unowned": adopt, "sources": sources, "requirements": []
    })
}

/// The lines of a state file that name the Thread.
fn thread_lines(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains(THREAD))
        .count()
}

#[test]
#[verifies("rule_typed_deletion_deletes_discussions", examples)]
fn a_discussion_with_messages_in_an_earlier_month_shard_is_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .args(["--path-prefix", "."])
        .assert()
        .success();
    // A native create opens a review submission, and typed-spec apply does not
    // delete a record that has one. The typed spec creates the Source instead.
    let legacy = json!([{"key":"legacy", "id":"source_legacy", "name":"Legacy", "kind":"policy"}]);
    provenance(
        repo,
        &["api", "authoring-changes", "--method", "post"],
        Some(spec(&legacy, &json!([]))),
    );
    // No writer makes a new month shard, but readers accept the shards that
    // earlier builds wrote.
    let threads = repo.join(".provenance/state/scopes/default/threads");
    std::fs::create_dir_all(&threads).unwrap();
    std::fs::write(
        threads.join("threads.jsonl"),
        format!(
            "{}\n",
            json!({
                "schema_version":2, "scope_id":"default", "id":THREAD,
                "parent":{"node_type":"source", "node_id":"source_legacy"},
                "status":"active", "created_at":1
            })
        ),
    )
    .unwrap();
    let september = threads.join("2026-09.jsonl");
    std::fs::write(
        &september,
        format!(
            "{}\n",
            json!({
                "schema_version":2, "scope_id":"default", "id":"msg_legacy",
                "thread_id":THREAD, "role":"user", "body":"An earlier note.",
                "created_at":1_789_814_529_628_u64
            })
        ),
    )
    .unwrap();
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "Legacy month shard"]);
    let started = provenance(
        repo,
        &[
            "api",
            "sources/source_legacy/discussions",
            "--method",
            "post",
        ],
        Some(json!({
            "actor":"reviewer", "declared_by":"spec://fx", "role":"user", "body":"A concern."
        })),
    );
    assert_eq!(started["data"]["thread_id"], THREAD, "{started}");

    let applied = provenance(
        repo,
        &["api", "authoring-changes", "--method", "post"],
        Some(spec(&json!([]), &json!([]))),
    );

    assert_eq!(applied["data"]["deleted"], 1, "{applied}");
    assert_eq!(thread_lines(&september), 0);
    assert_eq!(thread_lines(&threads.join("2026-07.jsonl")), 0);
    assert_eq!(thread_lines(&threads.join("threads.jsonl")), 0);
    let listed = provenance(repo, &["api", "messages"], None);
    assert_eq!(listed["data"]["items"], json!([]), "{listed}");
    provenance(repo, &["check"], None);
}
