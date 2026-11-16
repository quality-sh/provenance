//! A Withdrawal row that names no review submission makes the review rows of
//! the scope disagree. When the read database holds a stored revision, the get
//! of a record answers at that serial with a warning that names the file and
//! the line of the row, on the CLI and on a spawned `provenance mcp`. The api
//! read of the record, the review view and a write refuse with that row.

use provenance_macros::verifies;
use serde_json::{json, Value};

use crate::cli_get_damaged_state::run;
use crate::cli_mcp_read_parity::{chain, cli_json};
use crate::cli_mcp_record_actions::Server;

const RULE: &str = "rule_overtime_pay";
const WITHDRAWALS: &str = "scopes/default/ideation/withdrawals.jsonl";

/// Writes a Withdrawal that names no review submission as line 1 of the
/// Withdrawal file.
fn write_lost_withdrawal(root: &std::path::Path) {
    let path = root.join(".provenance/state").join(WITHDRAWALS);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let withdrawal = json!({"scope_id":"default","id":"withdrawal_lost",
        "proposal_id":"proposal_missing","actor":"reviewer","reason":null});
    std::fs::write(&path, format!("{withdrawal}\n")).unwrap();
}

/// The exit code, the standard output and the standard error of one CLI
/// command, with each path in the standard error written with `/`.
fn row(repo: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run(repo, args);
    json!({"command": args.join(" "), "code": code, "stdout": stdout,
        "stderr": stderr.replace('\\', "/")})
}

#[tokio::test]
async fn each_surface_over_a_withdrawal_that_names_no_submission() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let clean = cli_json(&repo, &[RULE, "get"]);
    let etag = clean["edit"]["etag"].as_str().unwrap().to_owned();
    write_lost_withdrawal(directory.path());
    let server = Server::start(&repo).await;

    let called = server.client.call("get", json!({"target":RULE})).await;
    let rows = json!([
        row(&repo, &[RULE, "get"]),
        row(&repo, &[RULE, "get", "--format", "json"]),
        {"command": "mcp get", "is_error": called["isError"],
            "text": called["content"][0]["text"]},
        row(&repo, &["api", &format!("rules/{RULE}"), "--format", "json"]),
        row(&repo, &[RULE, "--view", "review"]),
        row(&repo, &[RULE, "update", "--if-match", &etag, "--description", "Changed."]),
    ]);
    server.close().await;
    assert_eq!(rows, Value::Null);
}
