//! The api catalog through a spawned `provenance mcp` gives the same routes, in the same
//! order, and the same bounds as the CLI. Without a limit, MCP gives at most 50 routes, as
//! the CLI text answer does, and says how to see more.

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

#[tokio::test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
async fn mcp_catalog_reports_the_routes_and_bounds_of_the_cli() {
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
        let result = client.call("api", arguments.clone()).await;
        assert_ne!(result["isError"], json!(true), "{arguments}: {result}");
        assert_eq!(result["structuredContent"], cli_json(&args), "{arguments}");
    }
    let limited = client
        .call("api", json!({"filter": "withdraw", "limit": 2}))
        .await;
    let text = limited["content"][0]["text"].as_str().unwrap();
    assert!(
        text.ends_with("Use {\"filter\":\"<text>\"} or {\"limit\":<number>} to see more."),
        "{text}"
    );

    let total = all["bounds"]["total"].as_u64().unwrap();
    let unlimited = client.call("api", json!({})).await;
    let answer = &unlimited["structuredContent"];
    assert_eq!(
        answer["bounds"],
        json!({"total": total, "matching": total, "returned": 50, "truncated": true})
    );
    assert_eq!(
        answer["routes"].as_array().unwrap()[..],
        all["routes"].as_array().unwrap()[..50]
    );
    let text = unlimited["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("api routes: 50\n"), "{text}");
    assert!(text.ends_with("to see more."), "{text}");

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}
