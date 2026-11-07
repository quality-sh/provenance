//! The repository commands give the same result on the CLI and as change actions of a
//! spawned `provenance mcp`. Twin repositories run the same commands, one through the CLI
//! and one through MCP, and each standard output line of the server is JSON-RPC.

#[allow(dead_code)]
#[path = "cli_dictionary/support.rs"]
mod dictionary_support;
#[allow(dead_code)]
#[path = "export_fixture_support/mod.rs"]
mod export_fixture_support;
#[allow(dead_code)]
#[path = "cli_swarm_backtrace_land/support.rs"]
mod support;

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};

/// The JSON result of one CLI command that succeeds, or null when it prints nothing.
fn cli(index: &Path, args: &[&str]) -> Value {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .env("PROVENANCE_STE100_INDEX_DIR", index)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(&output.stdout).unwrap()
}

/// One spawned `provenance mcp` that the test speaks raw JSON-RPC to.
struct Wire {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
    next_id: u64,
}

impl Wire {
    async fn start(repo: &str, index: &Path) -> Self {
        let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .env("PROVENANCE_STE100_INDEX_DIR", index)
            .args(["mcp", "--repo", repo])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
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
                   "clientInfo":{"name":"cli_mcp_shared_commands","version":"0"}}),
        )
        .await;
        wire.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        wire
    }

    async fn send(&mut self, message: &Value) {
        let mut line = serde_json::to_vec(message).unwrap();
        line.push(b'\n');
        self.stdin.write_all(&line).await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    /// Sends one request and returns its result. Each line before the result must be
    /// one JSON-RPC message too.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await;
        loop {
            let line = self.lines.next_line().await.unwrap().unwrap();
            let mut message: Value = serde_json::from_str(&line)
                .unwrap_or_else(|error| panic!("stdout line is not JSON ({error}): {line}"));
            assert_eq!(message["jsonrpc"], "2.0", "not JSON-RPC: {line}");
            if message["id"] == json!(id) {
                return message["result"].take();
            }
        }
    }

    /// The structured result of one change action, without the action name. The text
    /// content is a readable line, not the JSON result.
    async fn change(&mut self, arguments: Value) -> Value {
        let result = self
            .request("tools/call", json!({"name":"change","arguments":arguments}))
            .await;
        assert_ne!(result["isError"], json!(true), "{result}");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(serde_json::from_str::<Value>(text).is_err(), "{text}");
        let mut structured = result["structuredContent"].clone();
        let named = structured.as_object_mut().unwrap().remove("action");
        assert_eq!(named, Some(arguments["action"].clone()), "{result}");
        structured
    }

    async fn close(mut self) {
        drop(self.stdin);
        while let Some(line) = self.lines.next_line().await.unwrap() {
            let message: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(message["jsonrpc"], "2.0", "not JSON-RPC: {line}");
        }
        assert!(self.child.wait().await.unwrap().success());
    }
}

#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", examples)]
#[verifies("rule_mcp_command_output_only_in_result", examples)]
async fn repository_actions_give_the_cli_result_on_mcp() {
    let directories = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let [by_cli, by_mcp] = directories
        .each_ref()
        .map(|directory| directory.path().to_str().unwrap().to_owned());
    let indexes = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let [cli_index, mcp_index] = indexes.each_ref().map(tempfile::TempDir::path);
    for repo in [&by_cli, &by_mcp] {
        support::init_repo(repo);
        support::create_source(repo);
        export_fixture_support::make_default_scope_portable(repo);
    }
    let files = tempfile::tempdir().unwrap();
    let export = files.path().join("export.json");
    let export = export.to_str().unwrap();
    let run_dir = files.path().join("run");
    support::write_run_dir(&run_dir, "The token is checked.");
    let run_dir = run_dir.to_str().unwrap();
    let pdf = files.path().join("dictionary.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();
    let pdf = pdf.to_str().unwrap();
    let repo = |repo: &str| ["--repo", repo, "--format", "json"].map(str::to_owned);
    let run = |args: &[&str], repo: &[String; 4]| {
        let mut all = args.to_vec();
        all.extend(repo.iter().map(String::as_str));
        cli(cli_index, &all)
    };
    let cli_repo = repo(&by_cli);
    run(&["export", "--output", export], &cli_repo);

    let mut wire = Wire::start(&by_mcp, mcp_index).await;
    let pairs = [
        (
            run(&["import", "--input", export], &cli_repo),
            json!({"action":"import","input":export}),
        ),
        (
            run(
                &["swarm-backtrace", "land", "--run-dir", run_dir],
                &cli_repo,
            ),
            json!({"action":"swarm-backtrace land","run_dir":run_dir}),
        ),
        (
            run(&["materialize"], &cli_repo),
            json!({"action":"materialize"}),
        ),
        (
            run(&["dictionary", "import", "--pdf", pdf], &cli_repo),
            json!({"action":"dictionary import","pdf":pdf}),
        ),
    ];
    for (mut by_cli, arguments) in pairs {
        let mut by_mcp = wire.change(arguments.clone()).await;
        // Whether a rebuild makes a new cache file depends on the reads before it.
        for result in [&mut by_cli, &mut by_mcp] {
            result.as_object_mut().unwrap().remove("cache_recreated");
        }
        assert_eq!(by_mcp, by_cli, "{arguments}");
    }
    wire.close().await;
}
