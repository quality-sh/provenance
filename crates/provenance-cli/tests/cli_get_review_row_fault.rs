//! A Withdrawal row that names no review submission makes the review rows of
//! the scope disagree. When the read database holds a stored revision, the get
//! of a record answers at that serial with a warning that names the file and
//! the line of the row, on the CLI and on a spawned `provenance mcp`. The api
//! read of the record, the review view and an update refuse with that row.

use provenance_macros::verifies;
use serde_json::{json, Value};

use crate::cli_get_damaged_state::{assert_stored_record, assert_update_refused, clean_get, run};
use crate::cli_mcp_read_parity::chain;
use crate::cli_mcp_record_actions::Server;

const RULE: &str = "rule_overtime_pay";
const WITHDRAWALS: &str = "scopes/default/ideation/withdrawals.jsonl";
const MESSAGE: &str = "withdrawal withdrawal_lost names no review submission: proposal_missing";
const REPAIR: &str = "To repair, remove ideation/withdrawals.jsonl line 1, then commit.";

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
fn answer(repo: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run(repo, args);
    json!({"code": code, "stdout": stdout, "stderr": stderr.replace('\\', "/")})
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_get_over_a_withdrawal_that_names_no_submission_warns_at_the_stored_serial() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let clean = clean_get(&repo);
    write_lost_withdrawal(directory.path());
    let server = Server::start(&repo).await;

    let warning = format!("{WITHDRAWALS} line 1: {MESSAGE}; {REPAIR}");
    assert_stored_record(&server, &repo, &clean, &warning).await;

    let refused = json!({"error":{"file":WITHDRAWALS,"hint":REPAIR,"kind":"invalid_state_file",
        "line":1,"message":MESSAGE},"meta":{}});
    let api = answer(
        &repo,
        &["api", &format!("rules/{RULE}"), "--format", "json"],
    );
    assert_eq!(
        api,
        json!({"code":1,"stdout":"","stderr":format!("Error: {refused}\n")})
    );
    assert_eq!(
        answer(&repo, &[RULE, "--view", "review"]),
        json!({"code":1,"stdout":"","stderr":format!("Error: {warning}\n")})
    );
    let etag = clean.0["edit"]["etag"].as_str().unwrap();
    assert_update_refused(&server, &repo, etag, &refused, &refused).await;
    server.close().await;
}
