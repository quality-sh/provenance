//! A spawned `provenance mcp` and the CLI take the same fields: the change tool and the
//! CLI record actions store the same records, and get returns the same review link.

use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::process::Stdio;

/// The standard output of one CLI command that succeeds.
fn cli(args: &[&str]) -> String {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn init(repo: &str) {
    cli(&[
        "init",
        "--scope",
        "default",
        "--path-prefix",
        ".",
        "--path",
        repo,
    ]);
}

/// One spawned `provenance mcp` process and its connected client.
struct Server {
    child: tokio::process::Child,
    client: Client,
}

impl Server {
    async fn start(repo: &str) -> Self {
        let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .args(["mcp", "--repo", repo])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let client = Client::connect(tokio::io::join(stdout, stdin))
            .await
            .unwrap();
        Self { child, client }
    }

    async fn close(mut self) {
        self.client.cancel().await.unwrap();
        let status = self.child.wait().await.unwrap();
        assert!(status.success(), "{status}");
    }
}

/// The stored value of one record, as the CLI reads it.
fn stored(repo: &str, id: &str) -> Value {
    let read = cli(&[id, "get", "--repo", repo, "--format", "json"]);
    let mut value = serde_json::from_str::<Value>(&read).unwrap()["record"]["value"].clone();
    for volatile in ["created_at", "updated_at", "edit"] {
        value.as_object_mut().unwrap().remove(volatile);
    }
    value
}

/// Gives `req_shared` the parent `req_p` through the CLI.
fn refine_shared(repo: &str) {
    let read: Value =
        serde_json::from_str(&cli(&["api", "requirements/req_shared", "--repo", repo])).unwrap();
    cli(&[
        "req_shared",
        "update",
        "--if-match",
        read["data"]["edit"]["etag"].as_str().unwrap(),
        "--actor",
        "author",
        "--relationships-json",
        r#"{"refines":["req_p"]}"#,
        "--repo",
        repo,
    ]);
}

/// One record action takes the same fields on the CLI and on MCP: twin repositories
/// that create Requirements and then move one parent, one through the CLI and one
/// through the change tool, store the same records.
#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", examples)]
async fn the_change_tool_and_the_cli_store_the_same_records() {
    let directories = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let [by_cli, by_mcp] = directories
        .each_ref()
        .map(|directory| directory.path().to_str().unwrap().to_owned());
    init(&by_cli);
    init(&by_mcp);
    let ids = ["req_p", "req_b", "req_shared"];
    let statement = "The system stores records.";
    for id in ids {
        cli(&[
            id,
            "create",
            "--type",
            "requirement",
            "--statement",
            statement,
            "--repo",
            &by_cli,
        ]);
    }
    let server = Server::start(&by_mcp).await;
    for id in ids {
        // The CLI fills these create fields with its own defaults; MCP takes them as given.
        let arguments = json!({"action":"create","target":id,"type":"requirement",
            "statement":statement,"actor":"cli","status":"active","depends_on":[],"supersedes":[]});
        let created = server.client.call("change", arguments).await;
        assert_ne!(created["isError"], json!(true), "{created}");
    }
    refine_shared(&by_cli);
    refine_shared(&by_mcp);
    cli(&[
        "req_shared",
        "update",
        "--actor",
        "author",
        "--replace-parent",
        "req_p",
        "--with-parent",
        "req_b",
        "--repo",
        &by_cli,
    ]);
    let arguments = json!({"action":"update","target":"req_shared","actor":"author",
        "replace_parent":"req_p","with_parent":"req_b"});
    let updated = server.client.call("change", arguments).await;
    assert_ne!(updated["isError"], json!(true), "{updated}");
    server.close().await;
    for id in ids {
        assert_eq!(stored(&by_cli, id), stored(&by_mcp, id), "{id}");
    }
    assert_eq!(stored(&by_mcp, "req_shared")["refines"], json!(["req_b"]));
}

/// Get with `review_link` returns the review link of the record on both surfaces. With no
/// review host, both give the text that tells how to start one.
#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", examples)]
async fn get_returns_the_same_review_link_on_both_surfaces() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    init(&repo);
    let statement = "The system stores records.";
    cli(&[
        "req_linked",
        "create",
        "--type",
        "requirement",
        "--statement",
        statement,
        "--repo",
        &repo,
    ]);
    let by_cli: Value = serde_json::from_str(&cli(&[
        "req_linked",
        "get",
        "--review-link",
        "--no-open",
        "--format",
        "json",
        "--repo",
        &repo,
    ]))
    .unwrap();
    let server = Server::start(&repo).await;
    let by_mcp = server
        .client
        .call("get", json!({"target":"req_linked","review_link":true}))
        .await;
    server.close().await;
    assert_ne!(by_mcp["isError"], json!(true), "{by_mcp}");
    let link = &by_mcp["structuredContent"]["review_link"];
    assert!(by_cli["message"].is_string(), "{by_cli}");
    assert_eq!(link["review_url"], by_cli["review_url"], "{by_mcp}");
    assert_eq!(link["message"], by_cli["message"], "{by_mcp}");
}
