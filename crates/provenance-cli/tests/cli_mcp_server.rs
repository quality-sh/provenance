//! `provenance mcp` over a spawned binary: the MCP tools for the repository that the CLI
//! commands open.

use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::process::Stdio;

/// The porcelain MCP tools, in list order.
const PORCELAIN_TOOL_NAMES: &[&str] = &[
    "get",
    "search",
    "api",
    "check",
    "create",
    "update",
    "answer",
    "claim",
    "release",
    "submit",
    "discussions",
    "discussion",
    "discuss",
    "reply",
];

fn binary() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

fn provenance() -> assert_cmd::Command {
    assert_cmd::Command::new(binary())
}

/// The standard output of one CLI command that succeeds.
fn cli(args: &[&str]) -> String {
    let output = provenance().args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn init_with_source(repo: &str) {
    cli(&[
        "init",
        "--scope",
        "default",
        "--path-prefix",
        ".",
        "--path",
        repo,
    ]);
    cli(&[
        "source_live_inventory",
        "create",
        "--type",
        "source",
        "--name",
        "Live inventory",
        "--repo",
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
        let mut child = tokio::process::Command::new(binary())
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

    /// Close the session and wait for a clean exit.
    async fn close(mut self) {
        self.client.cancel().await.unwrap();
        let status = self.child.wait().await.unwrap();
        assert!(status.success(), "{status}");
    }
}

/// The readable text of one tool result.
fn text(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

#[tokio::test]
#[verifies("rule_mcp_command_uses_cli_repository", examples)]
async fn mcp_serves_the_tools_of_the_cli_repository() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    init_with_source(&repo);
    let server = Server::start(&repo).await;
    let client = &server.client;

    let prime = cli(&["prime", "--repo", &repo]);
    assert_eq!(client.instructions().as_deref(), Some(prime.trim_end()));
    let tools = client.tools().await;
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, PORCELAIN_TOOL_NAMES);

    let mut results = Vec::new();
    let get = client
        .call("get", json!({"target":"source_live_inventory"}))
        .await;
    let cli_get = cli(&["source_live_inventory", "get", "--repo", &repo]);
    assert_eq!(text(&get), cli_get.trim_end());
    results.push(get);

    let check = client.call("check", json!({"categories":["graph"]})).await;
    let cli_check = cli(&["check", "--repo", &repo, "--graph"]);
    assert_eq!(text(&check), cli_check.trim_end());
    results.push(check);

    let created = client
        .call(
            "create",
            json!({"target":"source_made_over_mcp","type":"source","data":{"name":"Made over MCP","supersedes":[]}}),
        )
        .await;
    assert_ne!(created["isError"], json!(true), "{created}");
    results.push(created);
    let cli_made = cli(&[
        "source_made_over_mcp",
        "get",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let cli_made: Value = serde_json::from_str(&cli_made).unwrap();
    assert_eq!(cli_made["record"]["value"]["name"], "Made over MCP");

    let missing = client
        .call("api", json!({"path":"sources/source_missing"}))
        .await;
    assert_eq!(missing["isError"], json!(true), "{missing}");
    results.push(missing);
    results.push(client.call("search", json!({"text":"inventory"})).await);
    for result in &results {
        assert!(
            result["structuredContent"].is_object(),
            "structuredContent is a JSON object: {result}"
        );
    }
    server.close().await;
}

#[test]
#[verifies("rule_mcp_command_uses_cli_repository", examples)]
fn mcp_refuses_a_repository_without_a_graph() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    let output = provenance()
        .args(["mcp", "--repo", repo])
        .write_stdin("")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not initialized"), "{stderr}");
}
