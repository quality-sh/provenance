//! A text search page holds the matches that come after more than 512 records
//! that do not match. The repository has 513 Sources that do not hold the text
//! `needle`, then one Source that holds it. The first page holds that Source
//! and has no continuation on the CLI text form, the CLI JSON form, the MCP
//! `search` tool, and the HTTP route.
//!
//! These tests have no Rule, because no active Rule states that a page holds a
//! record when more records match.

#[path = "support/search_repository.rs"]
mod search_repository;

use provenance_mcp::Client;
use search_repository::{
    binary, cli, page, plain_sources, repository, search_json, source, source_node, without_stamp,
};
use serde_json::{json, Value};
use std::process::Stdio;

/// The repository of 513 plain Sources and then `source_needle`.
fn seed(directory: &std::path::Path) -> String {
    let mut sources = plain_sources(513);
    sources.push(source("source_needle", "Needle record"));
    repository(directory, sources, Vec::new())
}

#[test]
fn cli_first_text_search_page_holds_the_match_after_513_other_records() {
    let directory = tempfile::tempdir().unwrap();
    let repo = seed(directory.path());

    let first = search_json(&repo, &["--text", "needle"]);
    assert_eq!(first["next_cursor"], Value::Null, "{first}");
    assert_eq!(
        without_stamp(&first),
        page(
            &first["protocol_version"],
            50,
            false,
            vec![source_node("source_needle", "Needle record")]
        )
    );

    let text = cli(&["search", "--repo", &repo, "--text", "needle"]);
    assert_eq!(
        text,
        "search: 1 returned\n\
         - source source_needle\n  Needle record\n\
         bounds: limit=50 has_more=false continuation=none\n"
    );
}

#[test]
fn http_route_first_text_search_page_holds_the_match_after_513_other_records() {
    let directory = tempfile::tempdir().unwrap();
    let repo = seed(directory.path());

    let mut answer: Value = serde_json::from_str(&cli(&[
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
    assert!(answer["meta"]["stamp"].is_object(), "{answer}");
    answer["meta"]["stamp"] = Value::Null;
    assert_eq!(
        answer,
        json!({
            "data": {"items": [source_node("source_needle", "Needle record")]},
            "meta": {"has_more": false, "limit": 50, "next_cursor": null, "stamp": null}
        })
    );
}

#[tokio::test]
async fn mcp_first_text_search_page_holds_the_match_after_513_other_records() {
    let directory = tempfile::tempdir().unwrap();
    let repo = seed(directory.path());
    let by_cli = search_json(&repo, &["--text", "needle"]);
    assert_eq!(
        by_cli["nodes"],
        json!([source_node("source_needle", "Needle record")])
    );
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
