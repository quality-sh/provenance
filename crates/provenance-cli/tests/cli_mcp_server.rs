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
    "trace",
    "change",
    "discussions",
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
            "change",
            json!({"action":"create","target":"source_made_over_mcp","type":"source","data":{"name":"Made over MCP","source_type":"policy","url":null,"supersedes":[]}}),
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

/// One spawned `provenance mcp` process that the test speaks raw JSON-RPC to, so that it can
/// close standard input before it reads a result.
struct Wire {
    child: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
    next_id: u64,
}

impl Wire {
    async fn start(repo: &str) -> Self {
        use tokio::io::AsyncBufReadExt as _;
        let mut child = tokio::process::Command::new(binary())
            .args(["mcp", "--repo", repo])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let lines = tokio::io::BufReader::new(child.stdout.take().unwrap()).lines();
        let mut wire = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        wire.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},
                   "clientInfo":{"name":"cli_mcp_server","version":"0"}}),
        )
        .await;
        wire.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        wire
    }

    async fn send(&mut self, message: &Value) {
        use tokio::io::AsyncWriteExt as _;
        let mut line = serde_json::to_vec(message).unwrap();
        line.push(b'\n');
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(&line).await.unwrap();
        stdin.flush().await.unwrap();
    }

    /// Send one request without reading its response.
    async fn send_request(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await;
        id
    }

    /// Send one request and return its result.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params).await;
        loop {
            let line = self.lines.next_line().await.unwrap().unwrap();
            let mut message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == json!(id) {
                return message["result"].take();
            }
        }
    }

    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        let result = self
            .request("tools/call", json!({"name":name,"arguments":arguments}))
            .await;
        assert_ne!(result["isError"], json!(true), "{name}: {result}");
        result
    }
}

#[tokio::test]
#[verifies("rule_mcp_and_cli_read_each_other_writes", examples)]
#[verifies("rule_host_shutdown_completes_started_work", examples)]
async fn mcp_and_cli_read_each_other_writes_in_one_session() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    init_with_source(&repo);
    let mut wire = Wire::start(&repo).await;

    cli(&[
        "source_made_by_cli",
        "create",
        "--type",
        "source",
        "--name",
        "Made by the CLI",
        "--repo",
        &repo,
    ]);
    let get = wire
        .call("get", json!({"target":"source_made_by_cli"}))
        .await;
    let cli_get = cli(&["source_made_by_cli", "get", "--repo", &repo]);
    assert_eq!(text(&get), cli_get.trim_end());

    wire.call(
        "change",
        json!({"action":"update","target":"source_made_by_cli",
               "data":{"name":"Renamed over MCP"}}),
    )
    .await;
    let read = cli(&[
        "source_made_by_cli",
        "get",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let read: Value = serde_json::from_str(&read).unwrap();
    assert_eq!(read["record"]["value"]["name"], "Renamed over MCP");

    // Close standard input before the create result is read: the started write completes.
    wire.send_request(
        "tools/call",
        json!({"name":"change","arguments":{"action":"create","target":"source_made_at_close","type":"source",
               "data":{"name":"Made at close","source_type":"policy","url":null,"supersedes":[]}}}),
    )
    .await;
    drop(wire.stdin.take());
    let status = wire.child.wait().await.unwrap();
    assert!(status.success(), "{status}");
    let made = cli(&[
        "source_made_at_close",
        "get",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let made: Value = serde_json::from_str(&made).unwrap();
    assert_eq!(made["record"]["value"]["name"], "Made at close");
}

/// The discussions tool of a spawned `provenance mcp` lists its annotations, starts a
/// Discussion and replies to it, and the CLI reads that Discussion with its one read form.
#[tokio::test]
#[verifies("rule_mcp_one_tool_for_discussions", examples)]
#[verifies("rule_cli_one_discussion_read_form", examples)]
async fn mcp_discussions_tool_starts_and_replies_and_the_cli_reads_it() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    init_with_source(&repo);
    cli(&[
        "req_talk",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The system keeps review feedback available.",
    ]);
    let server = Server::start(&repo).await;
    let client = &server.client;

    let tools = client.tools().await;
    let discussions = tools
        .iter()
        .find(|tool| tool["name"] == "discussions")
        .unwrap();
    assert_eq!(
        discussions["annotations"],
        json!({"readOnlyHint":true,"openWorldHint":false})
    );
    let started = client
        .call(
            "change",
            json!({"action":"discuss","parent":{"node_type":"requirement","node_id":"req_talk"},
                   "actor":"ben","role":"user","body":"Is the wording right?"}),
        )
        .await;
    assert_ne!(started["isError"], json!(true), "{started}");
    let id = started["structuredContent"]["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let version = started["structuredContent"]["receipt"]["version"].clone();
    let replied = client
        .call(
            "change",
            json!({"action":"reply","discussion_id":id,"expected_version":version,
                   "actor":"agent","role":"assistant","body":"Yes, it is."}),
        )
        .await;
    assert_ne!(replied["isError"], json!(true), "{replied}");

    let read: Value = serde_json::from_str(&cli(&[
        &id,
        "discussion",
        "--repo",
        &repo,
        "--format",
        "json",
    ]))
    .unwrap();
    let text = read.to_string();
    assert!(text.contains("Is the wording right?"), "{read}");
    assert!(text.contains("Yes, it is."), "{read}");
    server.close().await;
}
