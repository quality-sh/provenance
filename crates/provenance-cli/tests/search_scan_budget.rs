//! A text search page reads at most 512 records. When none of the first 512
//! records holds the text, the first page has no record, `has_more` is true,
//! and the next page holds the match. This file shows that answer on the CLI
//! text form, the CLI JSON form, the MCP `search` tool, and the HTTP route.
//!
//! These tests have no Rule: no active Rule states what a page with no match
//! inside its scan budget must give, so they record the observed answer.

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

/// A repository with 513 Sources that do not hold the text `needle`, then
/// one Source that holds it. Id order puts the match after the first 512.
fn seed(directory: &std::path::Path) -> String {
    let repo = directory.join("repo");
    let repo_text = repo.to_str().unwrap().to_owned();
    cli(&[
        "init",
        "--path",
        &repo_text,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    let empty = directory.join("empty.json");
    cli(&[
        "export",
        "--repo",
        &repo_text,
        "--output",
        empty.to_str().unwrap(),
    ]);
    let mut document: Value = serde_json::from_slice(&std::fs::read(&empty).unwrap()).unwrap();
    let mut sources = (0..513)
        .map(|index| source(&format!("source_a{index:03}"), &format!("Plain {index:03}")))
        .collect::<Vec<_>>();
    sources.push(source("source_needle", "Needle record"));
    document["sources"] = Value::Array(sources);
    let input = directory.join("import.json");
    std::fs::write(&input, serde_json::to_vec(&document).unwrap()).unwrap();
    cli(&[
        "import",
        "--repo",
        &repo_text,
        "--input",
        input.to_str().unwrap(),
    ]);
    repo_text
}

fn source(id: &str, name: &str) -> Value {
    json!({"scope_id": "default", "id": id, "name": name, "source_type": "policy"})
}

fn search_json(repo: &str, extra: &[&str]) -> Value {
    let mut args = vec!["search", "--repo", repo, "--text", "needle"];
    args.extend(extra);
    args.extend(["--format", "json"]);
    serde_json::from_str(&cli(&args)).unwrap()
}

#[test]
fn cli_first_text_search_page_is_empty_with_more_pages() {
    let directory = tempfile::tempdir().unwrap();
    let repo = seed(directory.path());

    let first = search_json(&repo, &[]);
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    let mut expected = first.clone();
    expected["stamp"] = Value::Null;
    expected["next_cursor"] = Value::Null;
    assert_eq!(
        expected,
        json!({
            "protocol_version": first["protocol_version"],
            "operation": "search",
            "stamp": null,
            "next_cursor": null,
            "limit": 50,
            "has_more": true,
            "nodes": []
        })
    );

    let text = cli(&["search", "--repo", &repo, "--text", "needle"]);
    assert_eq!(
        text,
        format!("search: 0 returned\nbounds: limit=50 has_more=true continuation={cursor}\n")
    );

    let second = search_json(&repo, &["--cursor", &cursor]);
    assert_eq!(second["has_more"], false, "{second}");
    assert_eq!(second["next_cursor"], Value::Null, "{second}");
    assert_eq!(
        second["nodes"],
        json!([{
            "node_type": "source",
            "scope_id": "default",
            "id": "source_needle",
            "name": "Needle record",
            "source_type": "policy",
            "url": null
        }])
    );
}

#[test]
fn http_route_first_text_search_page_is_empty_with_more_pages() {
    let directory = tempfile::tempdir().unwrap();
    let repo = seed(directory.path());

    let answer: Value = serde_json::from_str(&cli(&[
        "api",
        "sources",
        "--repo",
        &repo,
        "--query",
        "query=search",
        "--query",
        "text=needle",
    ]))
    .unwrap();
    assert_eq!(answer["data"], json!({"items": []}), "{answer}");
    assert_eq!(answer["meta"]["has_more"], true, "{answer}");
    assert_eq!(answer["meta"]["limit"], 50, "{answer}");
    assert!(answer["meta"]["next_cursor"].is_string(), "{answer}");
}

#[tokio::test]
async fn mcp_first_text_search_page_is_the_cli_page() {
    let directory = tempfile::tempdir().unwrap();
    let repo = seed(directory.path());
    let by_cli = search_json(&repo, &[]);
    let text = cli(&["search", "--repo", &repo, "--text", "needle"]);

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

    let result = client.call("search", json!({"text": "needle"})).await;
    assert_ne!(result["isError"], json!(true), "{result}");
    assert_eq!(result["structuredContent"], by_cli);
    assert_eq!(
        result["content"],
        json!([{"type": "text", "text": text.trim_end()}])
    );

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}
