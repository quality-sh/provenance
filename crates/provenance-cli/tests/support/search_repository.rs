//! A repository of many Sources and Requirements, for the text search page tests.
// Each test binary that includes this file uses only some of the helpers.
#![allow(dead_code)]

use serde_json::{json, Value};
use std::path::Path;

pub fn binary() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// The standard output of one CLI command that succeeds.
pub fn cli(args: &[&str]) -> String {
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

/// One Source record of the import document.
pub fn source(id: &str, name: &str) -> Value {
    json!({"scope_id": "default", "id": id, "name": name, "source_type": "policy"})
}

/// One Requirement record of the import document.
pub fn requirement(id: &str, statement: &str) -> Value {
    json!({"scope_id": "default", "id": id, "statement": statement, "status": "active"})
}

/// The Sources `source_a000` and up, in id order, that do not hold the text
/// `needle`.
pub fn plain_sources(count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| source(&format!("source_a{index:03}"), &format!("Plain {index:03}")))
        .collect()
}

/// The search answer of one Source, as the CLI JSON form gives it.
pub fn source_node(id: &str, name: &str) -> Value {
    json!({
        "node_type": "source",
        "scope_id": "default",
        "id": id,
        "name": name,
        "source_type": "policy",
        "url": null
    })
}

/// The search answer of one Requirement, as the CLI JSON form gives it.
pub fn requirement_node(id: &str, statement: &str) -> Value {
    json!({
        "node_type": "requirement",
        "scope_id": "default",
        "id": id,
        "statement": statement,
        "status": "active"
    })
}

/// A new repository in `directory` that holds the given records.
pub fn repository(directory: &Path, sources: Vec<Value>, requirements: Vec<Value>) -> String {
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
    document["sources"] = Value::Array(sources);
    document["requirements"] = Value::Array(requirements);
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

/// One page of `search` in the CLI JSON form.
pub fn search_json(repo: &str, args: &[&str]) -> Value {
    let mut all = vec!["search", "--repo", repo];
    all.extend(args);
    all.extend(["--format", "json"]);
    serde_json::from_str(&cli(&all)).unwrap()
}

/// The page with the stamp and the cursor replaced by null, so that a test
/// can compare the complete page.
pub fn without_stamp(page: &Value) -> Value {
    let mut page = page.clone();
    page["stamp"] = Value::Null;
    page["next_cursor"] = Value::Null;
    page
}

/// The expected CLI JSON page, with the stamp and the cursor as null.
pub fn page(protocol_version: &Value, limit: usize, has_more: bool, nodes: Vec<Value>) -> Value {
    json!({
        "protocol_version": protocol_version,
        "operation": "search",
        "stamp": null,
        "next_cursor": null,
        "limit": limit,
        "has_more": has_more,
        "nodes": nodes
    })
}

/// Each page of one search, from the first page to the page with no
/// continuation. Each page except the last has `has_more` true and a cursor.
pub fn all_pages(repo: &str, args: &[&str]) -> Vec<Value> {
    let mut pages = vec![search_json(repo, args)];
    while let Some(cursor) = pages.last().unwrap()["next_cursor"].as_str() {
        assert_eq!(pages.last().unwrap()["has_more"], true);
        let cursor = cursor.to_owned();
        let mut next = args.to_vec();
        next.extend(["--cursor", cursor.as_str()]);
        pages.push(search_json(repo, &next));
        assert!(pages.len() < 100, "the search does not end");
    }
    assert_eq!(pages.last().unwrap()["has_more"], false);
    pages
}

/// The record ids of the pages, in page order.
pub fn ids(pages: &[Value]) -> Vec<String> {
    pages
        .iter()
        .flat_map(|page| page["nodes"].as_array().unwrap().iter())
        .map(|node| node["id"].as_str().unwrap().to_owned())
        .collect()
}
