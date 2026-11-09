//! Characterization: the read answers after each write and hand edit,
//! through the CLI process and through a spawned `provenance mcp`. The HTTP
//! routes run the same steps in the transport tests.
//!
//! A CLI process catches up the projection at its first read. The MCP
//! process lives across the steps, so it catches up the projection that it
//! wrote. Each answer must equal the answer of a full rebuild of the same
//! canonical state, value for value.

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/catch_up_steps.rs"]
    pub mod catch_up_steps;
    #[path = "../../../provenance-transport/tests/support/transcript.rs"]
    pub mod transcript;
}

use provenance_mcp::Client;
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Map, Value};
use std::io::Write as _;
use std::process::Stdio;
use support::catch_up_steps::{write, Answer, Surface, STEPS};
use support::transcript::{assert_same, copy_without_projection, read_all};

#[path = "../../provenance-store/tests/support/initialization.rs"]
mod initialization;

fn binary() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// The path without its leading `/`, and the query pairs of the path.
fn split(path: &str) -> (&str, Vec<(&str, &str)>) {
    let path = path.trim_start_matches('/');
    let Some((path, query)) = path.split_once('?') else {
        return (path, Vec::new());
    };
    let pairs = query
        .split('&')
        .map(|pair| pair.split_once('=').unwrap())
        .collect();
    (path, pairs)
}

/// The `provenance api` command, one process for each call.
struct Cli(String);

impl Cli {
    fn run(
        &self,
        method: &str,
        path: &str,
        data: Option<&Value>,
        headers: &[(&str, &str)],
    ) -> (u16, Value) {
        let (path, query) = split(path);
        let mut command = std::process::Command::new(binary());
        command.args([
            "api", path, "--repo", &self.0, "--format", "json", "--method", method,
        ]);
        for (name, value) in query {
            command.args(["--query", &format!("{name}={value}")]);
        }
        for (name, value) in headers {
            command.args(["--header", &format!("{name}: {value}")]);
        }
        if data.is_some() {
            command.args(["--input", "-"]);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        if let Some(data) = data {
            stdin.write_all(data.to_string().as_bytes()).unwrap();
        }
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        let text = |bytes: &[u8]| String::from_utf8(bytes.to_vec()).unwrap();
        if output.status.success() {
            (200, serde_json::from_slice(&output.stdout).unwrap())
        } else {
            let stderr = text(&output.stderr);
            let stderr = serde_json::from_str(&stderr).unwrap_or(Value::String(stderr));
            (1, json!({"stdout": text(&output.stdout), "stderr": stderr}))
        }
    }
}

impl Surface for Cli {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        data: Option<Value>,
        headers: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        let answer = self.run(method, path, data.as_ref(), headers);
        Box::pin(async move { answer })
    }
}

/// The `api` tool of one `provenance mcp` process.
struct Mcp {
    client: Client,
    _child: tokio::process::Child,
}

impl Mcp {
    async fn spawn(repo: &Repository) -> Self {
        let mut child = tokio::process::Command::new(binary())
            .args(["mcp", "--repo", repo.dir.path().to_str().unwrap()])
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
        Self {
            client,
            _child: child,
        }
    }
}

impl Surface for Mcp {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        data: Option<Value>,
        headers: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        Box::pin(async move {
            let (path, query) = split(path);
            let mut arguments = Map::new();
            arguments.insert("path".into(), json!(path));
            arguments.insert("method".into(), json!(method.to_ascii_lowercase()));
            if !query.is_empty() {
                let query: Map<String, Value> = query
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), json!(value)))
                    .collect();
                arguments.insert("query".into(), Value::Object(query));
            }
            if !headers.is_empty() {
                let headers: Map<String, Value> = headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), json!(value)))
                    .collect();
                arguments.insert("header".into(), Value::Object(headers));
            }
            if let Some(data) = data {
                arguments.insert("input".into(), data);
            }
            let result = self.client.call("api", Value::Object(arguments)).await;
            let status = if result["isError"] == json!(true) {
                1
            } else {
                200
            };
            let body = result.get("structuredContent").cloned().unwrap_or(result);
            (status, body)
        })
    }
}

fn fixture() -> Repository {
    let repo = Repository::new("The shared graph is readable.");
    initialization::allow_reviewer(&repo.layout);
    repo
}

fn state(repo: &Repository) -> std::path::PathBuf {
    repo.layout.state_dir().into_std_path_buf()
}

fn cli(repo: &Repository) -> Cli {
    Cli(repo.dir.path().to_str().unwrap().to_owned())
}

/// Implementation aid: characterization of the CLI read answers after each
/// change, made before the catch-up change. No Rule binds it.
#[tokio::test]
async fn cli_reads_after_each_write_equal_a_full_rebuild() {
    let repo = fixture();
    for step in 0..STEPS {
        write(&cli(&repo), &state(&repo), step).await;
        let caught_up = read_all(&cli(&repo), &repo).await;
        let copy = copy_without_projection(&repo);
        let rebuilt = read_all(&cli(&copy), &copy).await;
        assert_same(&caught_up, &rebuilt, &format!("CLI step {step}"));
    }
}

/// Implementation aid: characterization of the MCP read answers after each
/// change, made before the catch-up change. No Rule binds it.
#[tokio::test]
async fn mcp_reads_after_each_write_equal_a_full_rebuild() {
    let repo = fixture();
    let mcp = Mcp::spawn(&repo).await;
    for step in 0..STEPS {
        write(&mcp, &state(&repo), step).await;
        let caught_up = read_all(&mcp, &repo).await;
        let copy = copy_without_projection(&repo);
        let rebuilt = read_all(&Mcp::spawn(&copy).await, &copy).await;
        assert_same(&caught_up, &rebuilt, &format!("MCP step {step}"));
    }
}
