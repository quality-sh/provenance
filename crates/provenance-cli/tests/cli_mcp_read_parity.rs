//! The read commands give the same records on the CLI and through a spawned
//! `provenance mcp`: search, api, and get with `depth` and `kind`. Each MCP structured
//! result is the same JSON as the CLI `--format json` answer for the same input.

use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::process::Stdio;

fn binary() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// The standard output of one CLI command that succeeds.
fn cli(args: &[&str]) -> String {
    let output = assert_cmd::Command::new(binary())
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

/// The JSON answer of one CLI command in `repo`.
fn cli_json(repo: &str, args: &[&str]) -> Value {
    let mut all = args.to_vec();
    all.extend(["--repo", repo, "--format", "json"]);
    serde_json::from_str(&cli(&all)).unwrap()
}

/// Cites `source` from the Requirement `requirement`.
fn cite(repo: &str, requirement: &str, source: &str) {
    let record = cli_json(repo, &[requirement, "get"]);
    let etag = record["edit"]["etag"].as_str().unwrap().to_owned();
    assert_cmd::Command::new(binary())
        .args([requirement, "update", "--repo", repo, "--if-match", &etag])
        .arg("--stdin")
        .write_stdin(json!({"relationships":{"cites":[{"source_id":source}]}}).to_string())
        .assert()
        .success();
}

/// A Rule under a Requirement that cites a Source.
fn chain(repo: &str) {
    cli(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    cli(&[
        "source_award",
        "create",
        "--type",
        "source",
        "--repo",
        repo,
        "--name",
        "Overtime award",
    ]);
    cli(&[
        "req_overtime",
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--statement",
        "Overtime follows the award thresholds.",
    ]);
    cite(repo, "req_overtime", "source_award");
    cli(&[
        "rule_overtime_pay",
        "create",
        "--type",
        "rule",
        "--repo",
        repo,
        "--requirement-id",
        "req_overtime",
        "--statement",
        "Pay overtime after the threshold.",
        "--status",
        "active",
        "--severity",
        "high",
    ]);
}

/// The structured result of one successful MCP tool call.
async fn mcp(client: &Client, tool: &str, arguments: Value) -> Value {
    let result = client.call(tool, arguments.clone()).await;
    assert_ne!(result["isError"], json!(true), "{tool} {arguments}: {result}");
    result["structuredContent"].clone()
}

#[tokio::test]
#[verifies("rule_mcp_command_uses_cli_repository", examples)]
#[verifies("rule_shared_command_same_name_and_grammar", examples)]
async fn search_api_and_get_with_depth_and_kind_give_the_cli_records_on_mcp() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let mut child = tokio::process::Command::new(binary())
        .args(["mcp", "--repo", &repo])
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

    // Depth 2 and the source kind select the Source two steps above the Rule, and no
    // record between them.
    let by_cli = cli_json(
        &repo,
        &[
            "rule_overtime_pay",
            "get",
            "--view",
            "trace",
            "--depth",
            "2",
            "--kind",
            "source",
        ],
    );
    let by_mcp = mcp(
        &client,
        "get",
        json!({"target":"rule_overtime_pay","view":"trace","depth":2,"kind":["source"]}),
    )
    .await;
    assert_eq!(by_mcp, by_cli);
    let related = by_cli["related"].as_array().unwrap();
    let ids = related
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["source_award"], "{by_cli}");

    let by_cli = cli_json(&repo, &["search", "--text", "overtime", "--kind", "rule"]);
    let by_mcp = mcp(
        &client,
        "search",
        json!({"text":"overtime","kind":["rule"]}),
    )
    .await;
    assert_eq!(by_mcp, by_cli);
    assert!(by_cli.to_string().contains("rule_overtime_pay"), "{by_cli}");

    let by_cli = cli_json(&repo, &["api", "sources/source_award"]);
    let by_mcp = mcp(&client, "api", json!({"path":"sources/source_award"})).await;
    assert_eq!(by_mcp, by_cli);
    assert!(by_cli.to_string().contains("Overtime award"), "{by_cli}");

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}
