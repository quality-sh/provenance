//! The api catalog through a spawned `provenance mcp` gives the same routes, in the same
//! order, and the same bounds as the CLI. Without a limit, MCP gives at most 50 routes, as
//! the CLI text answer does, and says how to see more. An answer that lists many routes
//! stays small, and a filter that is the method and path of one route gives its schemas.

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

/// One temporary repository and a connected `provenance mcp` client on it.
async fn serve() -> (tempfile::TempDir, String, tokio::process::Child, Client) {
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
    (directory, repo, child, client)
}

#[tokio::test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
async fn mcp_catalog_reports_the_routes_and_bounds_of_the_cli() {
    let (_directory, repo, mut child, client) = serve().await;

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
        text.ends_with(concat!(
            "Use {\"filter\":\"<text>\"} or {\"limit\":<number>} to see more.\n",
            "Use {\"filter\":\"<method> <path>\"} to see the schemas of one route."
        )),
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
    assert!(
        text.ends_with(
            "to see more.\nUse {\"filter\":\"<method> <path>\"} to see the schemas of one route."
        ),
        "{text}"
    );

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}

/// The most characters in one whole api tool result that lists many routes: about
/// 10,000 tokens, well below the 25,000 tokens that Claude Code reads from one tool.
const LISTING_CHARACTERS: usize = 40_000;

#[tokio::test]
#[verifies("rule_api_catalog_schemas_only_for_one_route", examples)]
async fn mcp_catalog_listing_is_small_and_one_route_gives_its_schemas() {
    let (_directory, repo, mut child, client) = serve().await;

    let mut sizes = Vec::new();
    let mut listings = Vec::new();
    for arguments in [
        json!({}),
        json!({"filter": "review"}),
        json!({"filter": "withdraw", "limit": 2}),
    ] {
        let result = client.call("api", arguments.clone()).await;
        assert_ne!(result["isError"], json!(true), "{arguments}: {result}");
        let size = result.to_string().len();
        eprintln!("api {arguments}: {size} characters");
        sizes.push((arguments, size));
        listings.push(result);
    }
    for (arguments, size) in &sizes {
        assert!(*size < LISTING_CHARACTERS, "{arguments}: {size} characters");
    }
    for listing in &listings {
        let routes = listing["structuredContent"]["routes"].as_array().unwrap();
        assert!(routes.len() > 1, "{listing}");
        for route in routes {
            assert!(route.get("request_schema").is_none(), "{route}");
            for form in route["queries"].as_array().unwrap() {
                assert!(form.get("success_schema").is_none(), "{route}");
                assert!(form.get("failure_schema").is_none(), "{route}");
                for parameter in form["parameters"].as_array().unwrap() {
                    assert!(parameter.get("schema").is_none(), "{route}");
                }
            }
        }
        let text = listing["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("Use {\"filter\":\"<method> <path>\"} to see the schemas of one route."),
            "{text}"
        );
    }

    let member = client
        .call("api", json!({"filter": "GET /sources/{id}"}))
        .await;
    let answer = &member["structuredContent"];
    assert_eq!(answer["bounds"]["returned"], 1, "{answer}");
    let route = &answer["routes"][0];
    assert_eq!(
        (&route["method"], &route["path"]),
        (&json!("get"), &json!("/sources/{id}"))
    );
    let base = route["queries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["query"].is_null())
        .unwrap();
    assert!(
        base["success_schema"]["properties"]["data"].is_object(),
        "{base}"
    );
    assert!(base["failure_schema"].is_object(), "{base}");
    assert!(base["parameters"][0]["schema"].is_object(), "{base}");
    assert_eq!(
        *answer,
        cli_json(&[
            "api",
            "--repo",
            &repo,
            "--filter",
            "get /sources/{id}",
            "--format",
            "json",
        ])
    );

    let create = client.call("api", json!({"filter": "post /sources"})).await;
    let routes = create["structuredContent"]["routes"].as_array().unwrap();
    assert_eq!(routes.len(), 1, "{create}");
    assert!(routes[0]["request_schema"].is_object(), "{create}");

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}
