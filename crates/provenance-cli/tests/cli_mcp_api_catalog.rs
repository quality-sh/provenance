//! The api catalog with a filter and a limit gives the same routes, in the same order,
//! on the CLI and through a spawned `provenance mcp`.

use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::process::Stdio;

fn binary() -> &'static std::path::Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// The JSON answer of one CLI command that succeeds.
fn cli_json(args: &[&str]) -> Value {
    let output = assert_cmd::Command::new(binary())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The method and path of each route, in catalog order.
fn names(routes: &Value) -> Vec<String> {
    routes
        .as_array()
        .unwrap()
        .iter()
        .map(|route| format!("{} {}", route["method"], route["path"]))
        .collect()
}

/// The number of routes in one MCP catalog answer.
async fn count(client: &Client, arguments: Value) -> u64 {
    let result = client.call("api", arguments).await;
    u64::try_from(
        result["structuredContent"]["routes"]
            .as_array()
            .unwrap()
            .len(),
    )
    .unwrap()
}

#[tokio::test]
#[verifies("rule_cli_api_discovery_filter_limit", examples)]
async fn mcp_catalog_applies_the_filter_and_limit_of_the_cli() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    assert_cmd::Command::new(binary())
        .args(["init", "--path", &repo, "--scope", "default"])
        .assert()
        .success();
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

    let all = cli_json(&["api", "--repo", &repo, "--format", "json"]);
    let filtered = cli_json(&[
        "api", "--repo", &repo, "--filter", "withdraw", "--format", "json",
    ]);
    let matching = filtered["bounds"]["matching"].as_u64().unwrap();
    assert!(matching > 2, "{}", filtered["bounds"]);
    assert!(matching < all["bounds"]["total"].as_u64().unwrap());

    for (arguments, flags) in [
        (json!({"filter": "withdraw"}), vec!["--filter", "withdraw"]),
        (
            json!({"filter": "withdraw", "limit": 2}),
            vec!["--filter", "withdraw", "--limit", "2"],
        ),
    ] {
        let mut args = vec!["api", "--repo", &repo, "--format", "json"];
        args.extend(flags);
        let by_cli = cli_json(&args);
        let result = client.call("api", arguments.clone()).await;
        assert_ne!(result["isError"], json!(true), "{arguments}: {result}");
        let by_mcp = &result["structuredContent"]["routes"];
        assert_eq!(names(by_mcp), names(&by_cli["routes"]), "{arguments}");
        assert_eq!(by_mcp, &by_cli["routes"], "{arguments}");
    }
    assert_eq!(
        count(&client, json!({"filter": "withdraw"})).await,
        matching
    );
    assert_eq!(
        count(&client, json!({"filter": "withdraw", "limit": 2})).await,
        2
    );

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}
