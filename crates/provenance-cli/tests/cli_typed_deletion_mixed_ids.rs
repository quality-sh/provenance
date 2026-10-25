//! Typed-spec apply deletes a Discussion whose Thread holds sequential Message
//! ids from an earlier build in one month shard and random ids in another.
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

const DELETED: &str = "thread_source_source_legacy";
const KEPT: &str = "thread_source_source_kept";
const DISCUSSION: &str = "0b6c1f5e-8a4e-4c7d-9f0a-2d1e3c4b5a69";

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

fn git(repo: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
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

fn thread(id: &str, source: &str) -> String {
    json!({
        "schema_version":2, "scope_id":"default", "id":id,
        "parent":{"node_type":"source", "node_id":source},
        "status":"active", "created_at":1
    })
    .to_string()
}

/// A Message as earlier builds wrote it: the id is the zero-padded time.
fn sequential_message(created_at: u64, thread_id: &str) -> String {
    json!({
        "schema_version":2, "scope_id":"default", "id":format!("msg_{created_at:06}"),
        "thread_id":thread_id, "role":"user", "body":"An earlier note.",
        "created_at":created_at
    })
    .to_string()
}

fn discussion(id: &str, thread_id: &str, source: &str, message_ids: &[&str]) -> String {
    json!({
        "scope_id":"default", "discussion_id":id,
        "parent":{"node_type":"source", "node_id":source},
        "thread_id":thread_id, "root_message_id":message_ids[0], "message_ids":message_ids,
        "status":"active", "version":message_ids.len(), "actor":"reviewer", "outcomes":[],
        "disposition_id":null
    })
    .to_string()
}

fn cite(repo: &Path, requirement: &str, message_id: &str) {
    let created = provenance(
        repo,
        &["requirements", "create"],
        Some(json!({
            "actor":"reviewer", "id":requirement, "statement":"The host stops.",
            "status":"active", "depends_on":[], "supersedes":[],
            "origin":{"discussion_id":DISCUSSION, "thread_id":DELETED, "message_id":message_id}
        })),
    );
    assert_eq!(created["data"]["origin_message"], message_id, "{created}");
}

fn write_sequential_state(repo: &Path) -> std::path::PathBuf {
    let threads = repo.join(".provenance/state/scopes/default/threads");
    std::fs::create_dir_all(&threads).unwrap();
    std::fs::write(
        threads.join("threads.jsonl"),
        format!(
            "{}\n{}\n",
            thread(DELETED, "source_legacy"),
            thread(KEPT, "source_kept")
        ),
    )
    .unwrap();
    std::fs::write(
        threads.join("discussions.jsonl"),
        format!(
            "{}\n{}\n",
            discussion(
                DISCUSSION,
                DELETED,
                "source_legacy",
                &["msg_000001", "msg_000002"]
            ),
            discussion(
                "7d2e4f10-3b5a-4c6d-8e9f-0a1b2c3d4e5f",
                KEPT,
                "source_kept",
                &["msg_000003"]
            )
        ),
    )
    .unwrap();
    let september = threads.join("2026-09.jsonl");
    std::fs::write(
        &september,
        format!(
            "{}\n{}\n{}\n",
            sequential_message(1, DELETED),
            sequential_message(2, DELETED),
            sequential_message(3, KEPT)
        ),
    )
    .unwrap();
    september
}

#[test]
#[verifies("rule_typed_deletion_deletes_discussions", examples)]
#[verifies("rule_typed_deletion_clears_origins", examples)]
fn a_thread_with_sequential_and_random_message_ids_is_deleted() {
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
    for (id, name) in [("source_legacy", "Legacy"), ("source_kept", "Kept")] {
        provenance(
            repo,
            &["sources", "create", "--id", id, "--name", name],
            None,
        );
    }
    let september = write_sequential_state(repo);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "Sequential Message ids"]);
    let kept = json!({"key":"kept", "id":"source_kept", "name":"Kept", "kind":"policy"});
    provenance(
        repo,
        &["authoring-changes", "create"],
        Some(spec(
            &json!([
                {"key":"legacy", "id":"source_legacy", "name":"Legacy", "kind":"policy"},
                kept
            ]),
            &json!([
                {"kind":"source", "id":"source_legacy"},
                {"kind":"source", "id":"source_kept"}
            ]),
        )),
    );
    let replied = provenance(
        repo,
        &[
            DISCUSSION,
            "reply",
            "--expected-version",
            "2",
            "--body",
            "A reply.",
        ],
        None,
    );
    let random = replied["receipt"]["message_ids"][2]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        uuid::Uuid::parse_str(random.strip_prefix("msg_").unwrap()).is_ok(),
        "{replied}"
    );
    cite(repo, "req_from_sequential", "msg_000002");
    cite(repo, "req_from_random", &random);

    let applied = provenance(
        repo,
        &["authoring-changes", "create"],
        Some(spec(&json!([kept]), &json!([]))),
    );

    assert_eq!(applied["data"]["deleted"], 1, "{applied}");
    for requirement in ["req_from_sequential", "req_from_random"] {
        let cleared = provenance(repo, &["requirements", requirement, "get"], None);
        assert_eq!(cleared["data"]["origin_thread"], Value::Null, "{cleared}");
        assert_eq!(cleared["data"]["origin_message"], Value::Null, "{cleared}");
    }
    // The September shard is rewritten with the other Thread's Message only,
    // and the full read after the rewrite finds no duplicate id.
    let listed = provenance(repo, &["messages", "list"], None);
    let ids = listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["msg_000003"], "{listed}");
    let rewritten = std::fs::read_to_string(&september).unwrap();
    assert_eq!(rewritten.lines().count(), 1, "{rewritten}");
    assert!(rewritten.contains("msg_000003"), "{rewritten}");
    provenance(repo, &["check"], None);
}
