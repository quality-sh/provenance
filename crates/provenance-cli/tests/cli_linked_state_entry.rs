//! A link where a record file must be stops a read and a change with an error
//! of no known kind. The CLI `get`, `trace` and `search` and the MCP `get`,
//! `trace`, `change`, `discussions` and `search` of a spawned `provenance mcp`
//! give the cause of that error with the path of the link relative to the
//! repository, and no absolute path of the repository.
#![cfg(unix)]

use crate::bounded_process::{provenance, Bounded as _};
use crate::cli_mcp_record_actions::{init, Server};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

const LINK: &str = ".provenance/state/scopes/default/requirements/req.jsonl";
const CAUSE: &str =
    "unsupported state entry: .provenance/state/scopes/default/requirements/req.jsonl";

/// The exit code, the standard output and the standard error of one CLI command.
fn run(repo: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let output = provenance()
        .args(args)
        .args(["--repo", repo])
        .bounded_output();
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
    (
        output.status.code(),
        text(output.stdout),
        text(output.stderr),
    )
}

/// A repository with one Requirement that a get read one time, so that the
/// projection holds a revision.
fn read_once(repo: &str) {
    init(repo);
    let created = run(
        repo,
        &["req_alpha", "create", "--type", "requirement", "--statement", "The host starts."],
    );
    assert_eq!(created.0, Some(0), "{created:?}");
    let read = run(repo, &["req_alpha", "get"]);
    assert_eq!(read.0, Some(0), "{read:?}");
}

/// Moves the Requirement file out of the state directory and puts a link to it
/// in its place.
fn link_the_record_file(root: &Path) {
    let file = root.join(LINK);
    let copy = root.join("req.jsonl");
    std::fs::rename(&file, &copy).unwrap();
    std::os::unix::fs::symlink(&copy, &file).unwrap();
}

/// The answer of one MCP tool call: the error flag, the text and the
/// structured content.
async fn mcp(server: &Server, tool: &str, arguments: Value) -> Value {
    let called = server.client.call(tool, arguments).await;
    json!({"tool": tool, "isError": called["isError"],
        "text": called["content"][0]["text"], "structured": called["structuredContent"]})
}

#[tokio::test]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
async fn a_linked_record_file_gives_its_path_below_the_repository_on_each_read() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    read_once(&repo);
    let server = Server::start(&repo).await;
    link_the_record_file(directory.path());

    let mut cli = Vec::new();
    for args in [
        &["req_alpha", "get"][..],
        &["req_alpha", "trace"],
        &["search", "--text", "host"],
    ] {
        cli.push(run(&repo, args));
    }
    let mut answers = Vec::new();
    for (tool, arguments) in [
        ("get", json!({"target": "req_alpha"})),
        ("trace", json!({"target": "req_alpha"})),
        (
            "change",
            json!({"action": "update", "target": "req_alpha", "statement": "The host stops."}),
        ),
        ("discussions", json!({})),
        ("search", json!({"text": "host"})),
    ] {
        answers.push(mcp(&server, tool, arguments).await);
    }
    server.close().await;

    let refused = (Some(1), String::new(), format!("Error: {CAUSE}\n"));
    assert_eq!(cli, [refused.clone(), refused.clone(), refused]);
    let operation = json!({"error": {"kind": "operation_failed", "message": CAUSE}, "meta": {}});
    let read_failed = json!({"error": {"kind": "read_failed", "cause": CAUSE, "message": CAUSE},
        "meta": {}});
    let expected = [
        ("get", &operation),
        ("trace", &operation),
        ("change", &operation),
        ("discussions", &operation),
        ("search", &read_failed),
    ]
    .map(|(tool, structured)| {
        json!({"tool": tool, "isError": true, "text": structured.to_string(),
            "structured": structured})
    });
    assert_eq!(answers, expected);

    let canonical = dunce::canonicalize(directory.path()).unwrap();
    let output = format!("{cli:?}{answers:?}");
    for root in [directory.path(), canonical.as_path()] {
        let root = root.to_str().unwrap();
        assert!(!output.contains(root), "{root} in {output}");
    }
}
