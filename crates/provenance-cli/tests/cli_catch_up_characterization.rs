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
    pub mod api_process;
    #[path = "../../../provenance-transport/tests/support/catch_up_steps.rs"]
    pub mod catch_up_steps;
    #[path = "../../../provenance-transport/tests/support/transcript.rs"]
    pub mod transcript;
}

#[path = "support/bounded_process.rs"]
mod bounded_process;

use provenance_mcp::Client;
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Map, Value};
use std::process::Stdio;
use support::api_process;
use support::catch_up_steps::{write, Answer, Surface, STEPS};
use support::transcript::{assert_same, copy_without_projection, read_all};

#[path = "../../provenance-store/tests/support/initialization.rs"]
mod initialization;

fn binary() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// The `provenance api` command, one process for each call. Each call has
/// the time limit of `bounded_process`. A call that does not end in the
/// limit fails the test with the step, the side, the read or the write, and
/// the command.
struct Cli {
    repo: String,
    /// The step and the side of the test that make the calls.
    label: String,
}

impl Cli {
    fn run(
        &self,
        method: &str,
        path: &str,
        data: Option<&Value>,
        headers: &[(&str, &str)],
    ) -> (u16, Value) {
        let mut command =
            api_process::command(binary(), &self.repo, (method, path), headers, data.is_some());
        let kind = if method == "GET" { "read" } else { "write" };
        let what = format!("{}, {kind} {method} {path}", self.label);
        let input = data.map(|data| data.to_string().into_bytes());
        api_process::answer(&bounded_process::output_named(&mut command, input, &what))
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
            let (path, query) = api_process::split(path);
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

/// The CLI calls of `step` on `repo`, for the side of the test that `side`
/// names.
fn cli(repo: &Repository, step: usize, side: &str) -> Cli {
    Cli {
        repo: repo.dir.path().to_str().unwrap().to_owned(),
        label: format!("CLI step {step}, {side} side"),
    }
}

/// Implementation aid: characterization of the CLI read answers after each
/// change, made before the catch-up change. No Rule binds it.
#[tokio::test]
async fn cli_reads_after_each_write_equal_a_full_rebuild() {
    let repo = fixture();
    for step in 0..STEPS {
        write(&cli(&repo, step, "caught-up"), &state(&repo), step).await;
        let caught_up = read_all(&cli(&repo, step, "caught-up"), &repo).await;
        let copy = copy_without_projection(&repo);
        let rebuilt = read_all(&cli(&copy, step, "full-rebuild"), &copy).await;
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
