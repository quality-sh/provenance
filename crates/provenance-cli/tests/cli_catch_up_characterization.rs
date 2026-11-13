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
        let mut command = api_process::command(
            binary(),
            &self.repo,
            (method, path),
            headers,
            data.is_some(),
        );
        let what = named(&self.label, method, path);
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

/// The name of one call in a failure: `label`, the read or the write, the
/// method and the path.
fn named(label: &str, method: &str, path: &str) -> String {
    let kind = if method == "GET" { "read" } else { "write" };
    format!("{label}, {kind} {method} {path}")
}

/// Waits for `future` for the time limit of `bounded_process`. Past the
/// limit the test fails with `what`.
async fn within<T>(what: &str, future: impl std::future::Future<Output = T> + Send) -> T {
    let limit = bounded_process::LIMIT;
    tokio::time::timeout(limit, future)
        .await
        .unwrap_or_else(|_| panic!("{what}: no answer within {limit:?}"))
}

/// The `api` tool of one `provenance mcp` process. The process stops when
/// the value drops, also when a call fails the test at the time limit.
struct Mcp {
    client: Client,
    _child: tokio::process::Child,
}

impl Mcp {
    /// Starts `provenance mcp` on `repo` and connects to it within the time
    /// limit. A failure starts with `label`.
    async fn spawn(repo: &Repository, label: &str) -> Self {
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
        let connect = Client::connect(tokio::io::join(stdout, stdin));
        let what = format!("{label}, connect");
        let client = within(&what, connect).await.unwrap();
        Self {
            client,
            _child: child,
        }
    }

    /// The calls of the step and the side that `label` names.
    const fn side(&self, label: String) -> McpSide<'_> {
        McpSide { mcp: self, label }
    }
}

/// The calls of one step and one side through an [`Mcp`]. Each call has the
/// time limit of `bounded_process`.
struct McpSide<'a> {
    mcp: &'a Mcp,
    /// The step and the side of the test that make the calls.
    label: String,
}

impl Surface for McpSide<'_> {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        data: Option<Value>,
        headers: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        Box::pin(async move {
            let what = named(&self.label, method, path);
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
            let call = self.mcp.client.call("api", Value::Object(arguments));
            let result = within(&what, call).await;
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

/// Writes `label` to the output of the test when a panic drops this value.
struct Context<'a>(&'a str);

impl Drop for Context<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("the failure above is in {}", self.0);
        }
    }
}

/// Runs the writes of `step` through `surface`. The failure text of a
/// refused write gives no step and no side, so the output of the test then
/// also gives `label`.
async fn write_step(surface: &impl Surface, label: &str, state: &std::path::Path, step: usize) {
    let _context = Context(label);
    write(surface, state, step).await;
}

/// Implementation aid: characterization of the CLI read answers after each
/// change, made before the catch-up change. No Rule binds it.
#[tokio::test]
async fn cli_reads_after_each_write_equal_a_full_rebuild() {
    let repo = fixture();
    for step in 0..STEPS {
        let caught_up = cli(&repo, step, "caught-up");
        write_step(&caught_up, &caught_up.label, &state(&repo), step).await;
        let caught_up = read_all(&caught_up, &repo).await;
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
    let mcp = Mcp::spawn(&repo, "MCP before step 0, caught-up side").await;
    for step in 0..STEPS {
        let side = mcp.side(format!("MCP step {step}, caught-up side"));
        write_step(&side, &side.label, &state(&repo), step).await;
        let caught_up = read_all(&side, &repo).await;
        let copy = copy_without_projection(&repo);
        let label = format!("MCP step {step}, full-rebuild side");
        let rebuilt = Mcp::spawn(&copy, &label).await;
        let rebuilt = read_all(&rebuilt.side(label), &copy).await;
        assert_same(&caught_up, &rebuilt, &format!("MCP step {step}"));
    }
}
