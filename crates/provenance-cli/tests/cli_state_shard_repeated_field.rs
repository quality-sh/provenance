//! A Rule line that holds the field `id` two times is one fault of its line on
//! each surface, on the CLI and on a spawned `provenance mcp`. When the read
//! database holds a stored revision, the get, the search and the list read
//! answer at that serial with a warning that names the file and the line. The
//! api read of the record, the graph check and an update of a different Rule of
//! the same file refuse with that line. When the read database holds no
//! revision, the get refuses and names `provenance materialize`.

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use tempfile::TempDir;

use crate::cli_get_damaged_state::{
    assert_no_revision_refusal, assert_stored_record, clean_get, cli_refusal, mcp_refusal, run,
};
use crate::cli_mcp_read_parity::{chain, cli, cli_json, mcp};
use crate::cli_mcp_record_actions::Server;

const RULE: &str = "rule_overtime_pay";
const OTHER: &str = "rule_shift_rate";
const RULES: &str = "scopes/default/rules/rule.jsonl";
const MESSAGE: &str = "duplicate field `id`";

/// A repository with the records of `chain` and a second Rule in the same
/// file, whose read database holds a stored revision.
fn stored() -> (TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    cli(&[
        OTHER,
        "create",
        "--type",
        "rule",
        "--repo",
        &repo,
        "--requirement-id",
        "req_overtime",
        "--statement",
        "Pay the shift rate.",
        "--severity",
        "high",
    ]);
    (directory, repo)
}

/// Writes the field `id` a second time into the line of the Rule, before its
/// stored `id`, and gives the number of that line.
fn repeat_id(root: &Path) -> usize {
    let path = root.join(".provenance/state").join(RULES);
    let stored = std::fs::read_to_string(&path).unwrap();
    let field = format!("\"id\":\"{RULE}\"");
    let line = stored
        .lines()
        .position(|line| line.contains(&field))
        .unwrap();
    let changed = stored.replacen(&field, &format!("\"id\":\"rule_shadow\",{field}"), 1);
    std::fs::write(&path, changed).unwrap();
    line + 1
}

/// The text of the fault of the line `line`.
fn fault(line: usize) -> String {
    format!("{RULES} line {line}: {MESSAGE}; {STATE_CORRECTIONS}")
}

/// `clean` with the freshness error `warning` and the policy word
/// `catch_up_failed` in the object at the JSON pointer `at`.
fn warned(clean: &Value, at: &str, warning: &str) -> Value {
    let mut expected = clean.clone();
    let answer = expected.pointer_mut(at).unwrap();
    answer["freshness_error"] = json!(warning);
    answer["stamp"]["policy"] = json!("catch_up_failed");
    expected
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn the_get_the_search_and_the_list_read_warn_with_the_line_at_the_stored_serial() {
    let (directory, repo) = stored();
    let clean = clean_get(&repo);
    let search = ["search", "--text", "overtime", "--kind", "rule"];
    let searched = cli_json(&repo, &search);
    let listed = cli_json(&repo, &["api", "rules"]);
    let line = repeat_id(directory.path());
    let server = Server::start(&repo).await;

    assert_stored_record(&server, &repo, &clean, &fault(line)).await;
    let expected = warned(&searched, "", &fault(line));
    assert_eq!(cli_json(&repo, &search), expected);
    let arguments = json!({"text":"overtime","kind":["rule"]});
    assert_eq!(mcp(&server.client, "search", arguments).await, expected);
    server.close().await;
    assert_eq!(
        cli_json(&repo, &["api", "rules"]),
        warned(&listed, "/meta", &fault(line))
    );
}

/// An implementation aid with no Rule: no active Rule states what a read or a
/// write does with a line that holds a field two times.
#[tokio::test]
async fn the_api_read_the_graph_check_and_an_update_of_another_rule_refuse_with_the_line() {
    let (directory, repo) = stored();
    let etag = cli_json(&repo, &[OTHER, "get"])["edit"]["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    let line = repeat_id(directory.path());
    let rules = directory.path().join(".provenance/state").join(RULES);
    let damaged = std::fs::read(&rules).unwrap();
    let server = Server::start(&repo).await;

    let refused = json!({"error":{"file":RULES,"hint":STATE_CORRECTIONS,
        "kind":"invalid_state_file","line":line,"message":MESSAGE},"meta":{}});
    assert_eq!(
        cli_refusal(&repo, &["api", &format!("rules/{RULE}")]),
        refused
    );
    let (code, stdout, stderr) = run(&repo, &["check", "--graph"]);
    assert_eq!(
        (code, stdout.replace('\\', "/"), stderr.replace('\\', "/")),
        (
            Some(1),
            format!("graph: findings\n  - {}\n", fault(line)),
            format!("Error: {}\n", fault(line))
        )
    );
    let update = [
        OTHER,
        "update",
        "--if-match",
        &etag,
        "--description",
        "Changed.",
    ];
    assert_eq!(cli_refusal(&repo, &update), refused);
    let arguments = json!({"action":"update","target":OTHER,"if_match":etag,
        "description":"Changed."});
    assert_eq!(mcp_refusal(&server, "change", arguments).await, refused);
    server.close().await;
    assert_eq!(std::fs::read(&rules).unwrap(), damaged);
}

#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn the_get_with_no_stored_revision_refuses_with_the_line() {
    let (directory, repo) = stored();
    let line = repeat_id(directory.path());
    std::fs::remove_dir_all(directory.path().join(".provenance/cache")).unwrap();
    let server = Server::start(&repo).await;

    let by_cli = cli_refusal(&repo, &[RULE, "get"]);
    assert_no_revision_refusal(&by_cli, &fault(line));
    assert_eq!(
        mcp_refusal(&server, "get", json!({"target":RULE})).await,
        by_cli
    );
    server.close().await;
}
