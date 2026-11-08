//! A long-running HTTP host and a long-running MCP server give the answer
//! of a new CLI process, also when they answer the same range again.

use crate::answers::{commit_query, REQUESTS};
use crate::cases::{cases, RANGES};
use crate::review_host_support::{self, request, response};
use crate::support::{cli_error, cli_stale, fixture, git, remove_object};
use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;

/// One `provenance mcp` process for the repository.
struct Mcp {
    client: Client,
    child: tokio::process::Child,
}

impl Mcp {
    async fn start(repo: &Path) -> Self {
        let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .arg("mcp")
            .arg("--repo")
            .arg(repo)
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
        Self { client, child }
    }

    /// The structured result of an `api` call on `path`.
    async fn api(&self, path: &str, query: &[(&str, &str)]) -> Value {
        let query = query
            .iter()
            .map(|(name, value)| ((*name).to_string(), json!(value)))
            .collect::<serde_json::Map<_, _>>();
        self.client
            .call("api", json!({"path": path, "query": query}))
            .await
    }

    async fn stop(mut self) {
        self.client.cancel().await.unwrap();
        assert!(self.child.wait().await.unwrap().success());
    }
}

/// The body of an HTTP `GET` on `path` with the query.
fn http_get(host: &review_host_support::Host, path: &str, query: &[(&str, &str)]) -> Value {
    let mut call = request(host, "GET", path, true);
    for (name, value) in query {
        call = call.query(name, value);
    }
    let answer = response(call.call());
    assert_eq!(answer.status(), 200, "GET {path} {query:?}");
    serde_json::from_str(&answer.into_string().unwrap()).unwrap()
}

/// Each stale request and an evidence request with a base give the CLI
/// answer through HTTP and through MCP. Each host answers the requests
/// twice, so the second time it can use the scans that it kept.
#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_shared_command_same_result_on_cli_and_mcp", examples)]
async fn http_and_mcp_give_the_cli_answers() {
    let fixture = fixture();
    let host = review_host_support::start(fixture.path());
    let mcp = Mcp::start(fixture.path()).await;
    for _ in 0..2 {
        for item in &REQUESTS {
            let query = commit_query(&fixture, item);
            let query = query
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect::<Vec<_>>();
            let (code, cli, stderr) = cli_stale(fixture.path(), &query);
            assert_eq!(code, 0, "{stderr}");
            let mut stale = vec![("query", "stale")];
            stale.extend_from_slice(&query);
            let by_http = http_get(&host, "/rules", &stale);
            assert_eq!(by_http["data"], cli["data"], "HTTP {}", item.name);
            let by_mcp = mcp.api("rules", &stale).await;
            assert_ne!(by_mcp["isError"], json!(true), "{by_mcp}");
            assert_eq!(
                by_mcp["structuredContent"]["data"], cli["data"],
                "MCP {}",
                item.name
            );
        }
    }
    let evidence = [
        ("base", fixture.base.as_str()),
        ("head", fixture.head.as_str()),
    ];
    let (code, cli, stderr) =
        crate::support::cli_api(fixture.path(), "rules/rule_touched/evidence", &evidence);
    assert_eq!(code, 0, "{stderr}");
    assert!(!cli["data"]["stale"]["sites"].as_array().unwrap().is_empty());
    let by_http = http_get(&host, "/rules/rule_touched/evidence", &evidence);
    assert_eq!(by_http["data"], cli["data"]);
    let by_mcp = mcp.api("rules/rule_touched/evidence", &evidence).await;
    assert_eq!(by_mcp["structuredContent"]["data"], cli["data"]);
    mcp.stop().await;
}

/// The ranges of the cases give the CLI answer through a long-running MCP
/// server in a mixed order, so that a range follows a range that held the
/// same blobs or the same tree.
#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_shared_command_same_result_on_cli_and_mcp", examples)]
async fn mcp_gives_the_cli_answers_for_ranges_that_share_blobs() {
    let cases = cases();
    let mcp = Mcp::start(cases.path()).await;
    let expected = RANGES.map(|range| cases.cli_answer(range));
    for index in [0, 1, 2, 3, 1, 0, 2] {
        let (base, head) = RANGES[index];
        let query = [
            ("query", "stale"),
            ("base", cases.commits[base].as_str()),
            ("head", cases.commits[head].as_str()),
        ];
        let by_mcp = mcp.api("rules", &query).await;
        assert_eq!(
            by_mcp["structuredContent"]["data"], expected[index],
            "range {index}"
        );
    }
    mcp.stop().await;
}

/// After a blob of the range is lost, a server that answered the range
/// before gives the refusal of a new CLI process, with the read failure as
/// its cause.
#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_shared_command_same_result_on_cli_and_mcp", examples)]
#[verifies("rule_unclassified_refusal_gives_cause", examples)]
async fn a_warm_server_gives_the_cli_refusal_for_a_missing_blob() {
    let fixture = fixture();
    let mcp = Mcp::start(fixture.path()).await;
    let query = [
        ("query", "stale"),
        ("base", fixture.base.as_str()),
        ("head", fixture.head.as_str()),
    ];
    let warm = mcp.api("rules", &query).await;
    assert_ne!(warm["isError"], json!(true), "{warm}");

    let blob = git(
        fixture.path(),
        &["rev-parse", &format!("{}:src/touched.rs", fixture.head)],
    );
    remove_object(fixture.path(), &blob);
    let by_mcp = mcp.api("rules", &query).await;
    assert_eq!(by_mcp["isError"], json!(true), "{by_mcp}");
    let (code, _, stderr) = cli_stale(fixture.path(), &query[1..]);
    assert_eq!(code, 1, "{stderr}");
    let by_cli = cli_error(&stderr);
    assert_eq!(by_mcp["structuredContent"]["error"], by_cli);
    assert_eq!(by_cli["kind"], "read_failed", "{by_cli}");
    let cause = by_cli["cause"].as_str().unwrap();
    assert!(
        cause.contains("src/touched.rs") && cause.contains(&blob),
        "{cause}"
    );
    mcp.stop().await;
}
