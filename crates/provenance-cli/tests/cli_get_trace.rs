//! `provenance <id> trace` and `provenance <id> get --view trace` walk upward from one
//! record to its Sources, on the CLI and through a spawned `provenance mcp`.

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

/// Cites `source` from the Requirement `requirement`.
fn cite(repo: &str, requirement: &str, source: &str) {
    let store = provenance_store::repository::Repository::open(repo, "default")
        .unwrap()
        .store();
    let etag = store
        .requirement_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            &provenance_core::StableId::new(requirement).unwrap(),
        )
        .unwrap()
        .etag;
    drop(store);
    assert_cmd::Command::new(binary())
        .args([requirement, "update", "--repo", repo, "--if-match", &etag])
        .arg("--stdin")
        .write_stdin(json!({"relationships":{"cites":[{"source_id":source}]}}).to_string())
        .assert()
        .success();
}

/// A Rule under a Requirement that cites a Source, and an unrelated chain beside it.
fn chain() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    cli(&[
        "init",
        "--path",
        &repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    for (source, requirement, statement) in [
        (
            "source_award",
            "req_overtime",
            "Overtime follows the award thresholds.",
        ),
        (
            "source_unrelated",
            "req_unrelated",
            "Leave accrues each month.",
        ),
    ] {
        cli(&[
            source, "create", "--type", "source", "--repo", &repo, "--name", source,
        ]);
        cli(&[
            requirement,
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            statement,
        ]);
        cite(&repo, requirement, source);
    }
    cli(&[
        "rule_overtime_pay",
        "create",
        "--type",
        "rule",
        "--repo",
        &repo,
        "--requirement-id",
        "req_overtime",
        "--statement",
        "Pay overtime after the threshold.",
        "--status",
        "active",
        "--severity",
        "high",
    ]);
    (directory, repo)
}

fn related_ids(result: &Value) -> Vec<&str> {
    result["related"]
        .as_array()
        .unwrap()
        .iter()
        .map(|related| related["id"].as_str().unwrap())
        .collect()
}

/// The trace action and the trace view give the same records: the records above the
/// Rule up to its Source, and none of the unrelated chain.
#[test]
#[verifies("rule_trace_action_matches_trace_view", examples)]
fn trace_action_and_trace_view_give_the_same_records() {
    let (_directory, repo) = chain();
    let action = cli(&[
        "rule_overtime_pay",
        "trace",
        "--depth",
        "6",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let view = cli(&[
        "rule_overtime_pay",
        "get",
        "--view",
        "trace",
        "--depth",
        "6",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    assert_eq!(action, view);
    let result: Value = serde_json::from_str(&action).unwrap();
    assert_eq!(result["view"], "trace");
    let ids = related_ids(&result);
    assert!(ids.contains(&"req_overtime"), "{ids:?}");
    assert!(ids.contains(&"source_award"), "{ids:?}");
    assert!(!ids.contains(&"req_unrelated"), "{ids:?}");
    assert!(!ids.contains(&"source_unrelated"), "{ids:?}");

    let readable_action = cli(&["rule_overtime_pay", "trace", "--repo", &repo]);
    let readable_view = cli(&[
        "rule_overtime_pay",
        "get",
        "--view",
        "trace",
        "--repo",
        &repo,
    ]);
    assert_eq!(readable_action, readable_view);
}

/// The old view name is not a value of `--view`, and the trace action takes no view.
#[test]
#[verifies("rule_porcelain_read_rejects_bad_options", examples)]
fn grounding_view_and_trace_view_flag_are_refused() {
    let (_directory, repo) = chain();
    for args in [
        &["rule_overtime_pay", "get", "--view", "grounding"][..],
        &["rule_overtime_pay", "trace", "--view", "trace"][..],
    ] {
        let output = assert_cmd::Command::new(binary())
            .args(args)
            .args(["--repo", &repo])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{args:?}");
    }
}

/// The MCP trace tool, the MCP get trace view, and the CLI trace action give the same
/// result through a spawned `provenance mcp`.
#[tokio::test]
#[verifies("rule_trace_action_matches_trace_view", examples)]
async fn mcp_trace_tool_matches_trace_view_and_cli() {
    let (_directory, repo) = chain();
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

    let tool = client
        .call("trace", json!({"target":"rule_overtime_pay", "depth":6}))
        .await;
    let view = client
        .call(
            "get",
            json!({"target":"rule_overtime_pay", "view":"trace", "depth":6}),
        )
        .await;
    assert_ne!(tool["isError"], json!(true), "{tool}");
    assert_eq!(tool["structuredContent"], view["structuredContent"]);
    assert_eq!(tool["content"], view["content"]);
    let cli_trace = cli(&[
        "rule_overtime_pay",
        "trace",
        "--depth",
        "6",
        "--repo",
        &repo,
    ]);
    assert_eq!(tool["content"][0]["text"], cli_trace.trim_end());
    assert!(related_ids(&tool["structuredContent"]).contains(&"source_award"));

    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}

/// The children view walks down from a Requirement and the trace view walks up from it:
/// the cases of the removed `graph` and `traceability` commands.
#[test]
#[verifies("rule_porcelain_get_has_grounding_impact", examples)]
fn children_walks_down_and_trace_walks_up_from_a_requirement() {
    let (_directory, repo) = chain();
    let ids = |view: &str| -> Vec<String> {
        let output = cli(&[
            "req_overtime",
            "get",
            "--view",
            view,
            "--depth",
            "3",
            "--repo",
            &repo,
            "--format",
            "json",
        ]);
        let result: Value = serde_json::from_str(&output).unwrap();
        related_ids(&result)
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    let children = ids("children");
    assert!(
        children.contains(&"rule_overtime_pay".to_owned()),
        "{children:?}"
    );
    assert!(
        !children.contains(&"source_award".to_owned()),
        "{children:?}"
    );
    let trace = ids("trace");
    assert!(trace.contains(&"source_award".to_owned()), "{trace:?}");
    assert!(
        !trace.contains(&"rule_overtime_pay".to_owned()),
        "{trace:?}"
    );
    assert!(!trace.contains(&"source_unrelated".to_owned()), "{trace:?}");
}

/// Creates a Resolution under `requirement` and a Rule that the Resolution produces.
fn resolution_and_rule(repo: &str, requirement: &str, resolution: &str, rule: &str) {
    cli(&[
        resolution,
        "create",
        "--type",
        "resolution",
        "--status",
        "approved",
        "--repo",
        repo,
        "--title",
        resolution,
        "--requirement-id",
        requirement,
        "--position",
        "Use the award threshold.",
        "--rationale",
        "The award clause sets the threshold.",
    ]);
    cli(&[
        rule,
        "create",
        "--type",
        "rule",
        "--repo",
        repo,
        "--requirement-id",
        requirement,
        "--resolution-id",
        resolution,
        "--statement",
        "Pay overtime after the award threshold.",
        "--status",
        "active",
        "--severity",
        "high",
    ]);
}

/// The trace of a Rule that a Resolution produces contains the Resolution and its
/// Source, and none of the records of the other chain: the case of the removed
/// `traceability` command.
#[test]
#[verifies("rule_porcelain_get_has_grounding_impact", examples)]
fn trace_of_a_rule_contains_its_resolution_and_source() {
    let (_directory, repo) = chain();
    resolution_and_rule(&repo, "req_overtime", "res_overtime", "rule_overtime_award");
    resolution_and_rule(&repo, "req_unrelated", "res_unrelated", "rule_unrelated");
    let output = cli(&[
        "rule_overtime_award",
        "trace",
        "--depth",
        "6",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let result: Value = serde_json::from_str(&output).unwrap();
    let ids = related_ids(&result);
    for id in ["res_overtime", "req_overtime", "source_award"] {
        assert!(ids.contains(&id), "{id}: {ids:?}");
    }
    for id in [
        "res_unrelated",
        "rule_unrelated",
        "req_unrelated",
        "source_unrelated",
    ] {
        assert!(!ids.contains(&id), "{id}: {ids:?}");
    }
}
