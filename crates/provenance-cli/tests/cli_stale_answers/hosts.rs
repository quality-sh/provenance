//! A long-running HTTP host and a long-running MCP server give the answer
//! of a new CLI process, also when they answer the same range again.

use crate::answers::{commit_query, REQUESTS};
use crate::cases::{cases, RANGES};
use crate::review_host_support::{self, request, response};
use crate::support::{
    cli_error, cli_stale, commit, fixture, function, git, remove_object, write, Fixture,
};
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
#[verifies("rule_kept_scans_match_new_process", examples)]
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
    assert_ne!(cli["data"]["stale"]["sites"], json!([]));
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
#[verifies("rule_kept_scans_match_new_process", examples)]
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

/// A change to the commit that `HEAD` names after the hosts answered.
type Move = fn(&Fixture);

/// A warm HTTP host and a warm MCP server answer the stale query with no
/// head, then `change` moves `HEAD`. Their next answer is the answer of a
/// new CLI process, and it holds the site of the moved `rule_kept`.
async fn warm_hosts_follow_head(prepare: Move, change: Move) {
    let fixture = fixture();
    prepare(&fixture);
    let host = review_host_support::start(fixture.path());
    let mcp = Mcp::start(fixture.path()).await;
    let query = [("query", "stale"), ("base", fixture.base.as_str())];
    let warm = http_get(&host, "/rules", &query);
    let by_mcp = mcp.api("rules", &query).await;
    assert_eq!(by_mcp["structuredContent"]["data"], warm["data"]);
    assert_eq!(warm["data"]["head"], json!(fixture.head));

    change(&fixture);
    let (code, cli, stderr) = cli_stale(fixture.path(), &query[1..]);
    assert_eq!(code, 0, "{stderr}");
    assert_ne!(cli["data"]["head"], warm["data"]["head"]);
    assert!(
        cli["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|site| site["subject_id"] == "rule_kept" && site["line"] == 3),
        "{cli}"
    );
    let by_http = http_get(&host, "/rules", &query);
    assert_eq!(by_http["data"], cli["data"], "HTTP");
    let by_mcp = mcp.api("rules", &query).await;
    assert_ne!(by_mcp["isError"], json!(true), "{by_mcp}");
    assert_eq!(by_mcp["structuredContent"]["data"], cli["data"], "MCP");
    mcp.stop().await;
}

/// Moves the `rule_kept` binding down two lines and changes its body, so
/// that the site line comes from the scan of the new head.
fn change_kept(fixture: &Fixture) {
    write(
        fixture.path(),
        "src/kept.rs",
        format!("fn first() {{}}\n\n{}", function("rule_kept", "kept", "5")),
    );
}

const fn no_preparation(_: &Fixture) {}

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_kept_scans_match_new_process", examples)]
async fn warm_hosts_follow_a_new_commit() {
    warm_hosts_follow_head(no_preparation, |fixture| {
        change_kept(fixture);
        commit(fixture.path(), "later head");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_kept_scans_match_new_process", examples)]
async fn warm_hosts_follow_an_amended_commit() {
    warm_hosts_follow_head(no_preparation, |fixture| {
        change_kept(fixture);
        git(fixture.path(), &["add", "-A", "."]);
        git(fixture.path(), &["commit", "-q", "--amend", "--no-edit"]);
    })
    .await;
}

/// The branch `other` starts at the base and changes `src/kept.rs`.
#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_kept_scans_match_new_process", examples)]
async fn warm_hosts_follow_a_checkout_of_another_branch() {
    warm_hosts_follow_head(
        |fixture| {
            let repo = fixture.path();
            let branch = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
            git(repo, &["checkout", "-q", "-b", "other", &fixture.base]);
            change_kept(fixture);
            commit(repo, "other branch");
            git(repo, &["checkout", "-q", &branch]);
        },
        |fixture| {
            git(fixture.path(), &["checkout", "-q", "other"]);
        },
    )
    .await;
}
