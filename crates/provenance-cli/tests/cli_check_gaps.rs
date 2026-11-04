//! `provenance check --gaps` and MCP `check {categories:["gaps"]}`: one finding for each
//! unfinished part of the graph and one for each Rule without a Source, with the graph
//! counts of each scope.

use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::process::Stdio;

mod support {
    pub mod review_paths;
}
use support::review_paths::{init, init_scope, provenance};

/// The standard output of one CLI command that succeeds.
fn cli(args: &[&str]) -> String {
    let output = provenance().args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn requirement(repo: &str, scope: &str, id: &str) {
    cli(&[
        id,
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--scope",
        scope,
        "--statement",
        "The system keeps review feedback available.",
    ]);
}

/// Cites a new Source from the Requirement `id`.
fn cite(repo: &str, id: &str) {
    let source = format!("source_{id}");
    cli(&[
        &source, "create", "--type", "source", "--repo", repo, "--name", &source,
    ]);
    let record: Value =
        serde_json::from_str(&cli(&[id, "get", "--repo", repo, "--format", "json"])).unwrap();
    provenance()
        .args([id, "update", "--repo", repo, "--stdin", "--if-match"])
        .arg(record["edit"]["etag"].as_str().unwrap())
        .write_stdin(json!({"relationships":{"cites":[{"source_id":source}]}}).to_string())
        .assert()
        .success();
}

fn rule(repo: &str, scope: &str, id: &str, requirements: &[&str]) {
    cli(&[
        id,
        "create",
        "--type",
        "rule",
        "--repo",
        repo,
        "--scope",
        scope,
        "--requirement-ids-json",
        &json!(requirements).to_string(),
        "--statement",
        "The system keeps each review message.",
    ]);
}

/// The gaps category report of `check --gaps` with `args`. The run succeeds.
fn gaps(repo: &str, args: &[&str]) -> Value {
    let mut command = vec!["check", "--gaps", "--repo", repo, "--format", "json"];
    command.extend(args);
    let report: Value = serde_json::from_str(&cli(&command)).unwrap();
    let categories = report["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1, "{report}");
    categories[0].clone()
}

fn details(report: &Value) -> Vec<Value> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["detail"].clone())
        .collect()
}

fn orphans(report: &Value) -> Vec<String> {
    details(report)
        .iter()
        .filter_map(|detail| detail["rule_id"].as_str().map(str::to_owned))
        .collect()
}

fn missing_source_gaps(report: &Value) -> Vec<String> {
    details(report)
        .iter()
        .filter(|detail| detail["kind"] == "missing_source_refs")
        .map(|detail| detail["node_id"].as_str().unwrap().to_owned())
        .collect()
}

/// A Rule that two Requirements without a Source produce gives one orphan finding, and
/// each Requirement gives its own gap. The findings do not refuse, also with --strict,
/// and MCP gives the same findings.
#[tokio::test]
#[verifies("rule_check_reports_gaps", examples)]
async fn an_orphan_rule_gives_one_finding_and_each_requirement_its_gap() {
    let (_directory, repo) = init();
    requirement(&repo, "default", "req_a");
    requirement(&repo, "default", "req_b");
    rule(&repo, "default", "rule_r", &["req_a", "req_b"]);

    let report = gaps(&repo, &[]);
    assert_eq!(report["status"], "findings", "{report}");
    assert_eq!(orphans(&report), ["rule_r"], "{report}");
    let mut missing = missing_source_gaps(&report);
    missing.sort();
    assert_eq!(missing, ["req_a", "req_b"], "{report}");
    for detail in details(&report) {
        let item = detail.as_object().unwrap();
        assert!(
            item.contains_key("rule_id") || item.contains_key("node_id"),
            "{detail}"
        );
    }
    assert_eq!(gaps(&repo, &["--strict"])["findings"], report["findings"]);

    let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
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
    let mcp = client.call("check", json!({"categories":["gaps"]})).await;
    assert_ne!(mcp["isError"], json!(true), "{mcp}");
    assert_eq!(mcp["structuredContent"]["categories"][0], report);
    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}

/// A Rule that one Requirement with a Source produces is not an orphan; the other
/// Requirement still gives its gap.
#[test]
#[verifies("rule_check_reports_gaps", examples)]
fn a_rule_with_one_sourced_requirement_is_not_an_orphan() {
    let (_directory, repo) = init();
    requirement(&repo, "default", "req_a");
    requirement(&repo, "default", "req_b");
    cite(&repo, "req_a");
    rule(&repo, "default", "rule_r", &["req_a", "req_b"]);

    let report = gaps(&repo, &[]);
    assert!(orphans(&report).is_empty(), "{report}");
    assert_eq!(missing_source_gaps(&report), ["req_b"], "{report}");
}

/// A graph with no gaps passes and still gives the counts of its scope.
#[test]
#[verifies("rule_check_reports_gaps", examples)]
fn a_graph_without_gaps_passes_with_counts() {
    let (_directory, repo) = init();
    let report = gaps(&repo, &[]);
    assert_eq!(report["status"], "passed", "{report}");
    assert_eq!(
        report["context"],
        json!({"scopes":[{
            "scope_id":"default",
            "requirements":0,
            "source_linked_requirements":0,
            "resolved_requirements":0,
            "requirements_with_rules":0,
            "rules_total":0,
            "rules_with_complete_traceability":0
        }]})
    );
    assert!(!cli(&["check", "--repo", &repo, "--format", "json"]).contains("\"gaps\""));
}

/// With --scope, the findings and counts are of that scope only. Without it, each
/// declared scope has its counts and the findings name their scope.
#[test]
#[verifies("rule_check_scope_selects_category_records", examples)]
fn a_selected_scope_gives_only_its_findings_and_counts() {
    let (directory, repo) = init();
    init_scope(directory.path(), "app", "app");
    requirement(&repo, "default", "req_a");
    rule(&repo, "default", "rule_a", &["req_a"]);

    let app = gaps(&repo, &["--scope", "app"]);
    assert_eq!(app["status"], "passed", "{app}");
    assert_eq!(app["context"]["scopes"].as_array().unwrap().len(), 1);
    assert_eq!(app["context"]["scopes"][0]["scope_id"], "app");
    assert_eq!(app["context"]["scopes"][0]["rules_total"], 0);

    let selected = gaps(&repo, &["--scope", "default"]);
    assert_eq!(orphans(&selected), ["rule_a"], "{selected}");
    assert_eq!(selected["context"]["scopes"][0]["rules_total"], 1);

    let every = gaps(&repo, &[]);
    let scopes = every["context"]["scopes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|counts| counts["scope_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(scopes, ["default", "app"], "{every}");
    assert_eq!(every["findings"], selected["findings"]);
    assert!(every["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|finding| finding["message"]
            .as_str()
            .unwrap()
            .starts_with("default: ")));
}
