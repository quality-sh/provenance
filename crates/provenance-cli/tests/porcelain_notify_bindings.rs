//! The CLI and MCP register a Codex thread with one review host and refuse a bad
//! registration with the same `{error, meta}` object. The MCP host runs in a child process,
//! so the launch key and Codex home come from that child and not from this test process.
#![cfg(target_os = "linux")]

#[path = "review_host_support/codex.rs"]
mod codex;
#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    follow::{create, page_rule},
    Host,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const SOCKET: &str = "PROVENANCE_TEST_MCP_SOCKET";
const REPOSITORY: &str = "PROVENANCE_TEST_MCP_REPOSITORY";

/// The MCP host process. It stops the child when the test ends.
struct McpProcess(Child);

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `mcp_host_process` in a new process of this test binary. The child gets the
/// per-user cache of the review host and the Codex home through its environment.
fn mcp_process(host: &Host, codex: &codex::Codex, repo: &Path, socket: &Path) -> McpProcess {
    let mut command = Command::new(std::env::current_exe().unwrap());
    for (key, value) in host.cli().get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    codex.configure(&mut command);
    command
        .env(SOCKET, socket)
        .env(REPOSITORY, repo)
        .args(["mcp_host_process", "--exact", "--ignored", "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    McpProcess(command.spawn().unwrap())
}

async fn connect(socket: &Path) -> tokio::net::UnixStream {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match tokio::net::UnixStream::connect(socket).await {
            Ok(stream) => return stream,
            Err(error) if Instant::now() > deadline => panic!("MCP host process: {error}"),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

/// Serves MCP for one connection on the socket that the parent test names.
#[test]
#[ignore = "cli_and_mcp_notify_share_one_registration runs this as its MCP host process"]
fn mcp_host_process() {
    let Some(socket) = std::env::var_os(SOCKET) else {
        return;
    };
    let repo = std::env::var(REPOSITORY).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let listener = tokio::net::UnixListener::bind(&socket).unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let context = provenance_cli::repo_context::RepoContext::new(repo.as_str(), "default");
            let host = context.local_host().unwrap();
            let _ = host.serve_mcp(stream).await.unwrap().waiting().await;
        });
}

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_review_notify_cli_mcp_same", examples)]
async fn cli_and_mcp_notify_share_one_registration() {
    let repo = review_host_support::repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_notify", "requirement", &[]);
    let codex = codex::Codex::start();
    let thread = codex.thread(repo.path());
    let host = review_host_support::start_with(repo.path(), |command| {
        command.arg("--no-open");
        codex.configure(command);
    });
    let sockets = tempfile::tempdir().unwrap();
    let socket = sockets.path().join("mcp.sock");
    let _process = mcp_process(&host, &codex, repo.path(), &socket);
    let client = ().serve(connect(&socket).await).await.unwrap();
    let cli = |notify: &str, thread: &str| {
        codex::notify(&host, &codex, repo.path(), "req_notify", notify, thread)
    };
    let mcp = |notify: &'static str, thread: &str| {
        let arguments = json!({"target":"req_notify", "view":"review",
            "notify": {"client": notify, "thread": thread}});
        let call = client.call_tool(
            CallToolRequestParams::new("get")
                .with_arguments(arguments.as_object().unwrap().clone()),
        );
        async move { call.await.unwrap().structured_content.unwrap() }
    };

    let by_cli = codex::registration(&cli("codex", &thread));
    let removed_by_mcp = mcp("off", &thread).await["notify"].clone();
    page_rule(&host, "rule_after_mcp_off", "req_notify");
    let by_mcp = mcp("codex", &thread).await["notify"].clone();
    let removed_by_cli = codex::registration(&cli("off", &thread));
    page_rule(&host, "rule_after_cli_off", "req_notify");
    let refused_by_cli = cli("codex", "my-session");
    let refused_by_mcp = mcp("codex", "my-session").await;

    assert_eq!(by_cli, by_mcp);
    assert_eq!(by_cli["accepted"], true);
    assert_eq!(removed_by_cli, removed_by_mcp);
    assert_eq!(removed_by_cli["removed"], true);
    assert!(!refused_by_cli.status.success());
    let refused_by_cli: Value = serde_json::from_slice(&refused_by_cli.stderr).unwrap();
    assert_eq!(refused_by_cli["error"]["kind"], "invalid_input");
    assert_eq!(refused_by_cli["error"]["field"], "notify.thread");
    assert_eq!(refused_by_mcp["error"], refused_by_cli["error"]);
    std::thread::sleep(Duration::from_millis(1500));
    let queued = codex.queued(&thread);
    assert_eq!(queued.len(), 2, "{queued:#?}");
    assert!(queued[1].contains("rule_after_mcp_off"));
    assert!(!queued.concat().contains("rule_after_cli_off"));
    client.cancel().await.unwrap();
}
