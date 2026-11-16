//! A Withdrawal row that names no review submission makes the review rows of
//! the scope disagree. When the read database holds a stored revision, the get
//! of a record answers at that serial with a warning that names the file and
//! the line of the row, on the CLI and on a spawned `provenance mcp`. The api
//! read of the record, the review view and an update refuse with that row, and
//! the document read of a Requirement refuses with the failed catch-up. When the read database
//! holds no revision, the get and `provenance materialize` refuse.

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
    let document = answer(
        &repo,
        &[
            "api",
            "requirements/req_overtime/document",
            "--format",
            "json",
        ],
    );
    let catch_up = json!({"error":{"file":WITHDRAWALS,"hint":REPAIR,
        "kind":"document_catch_up_failed","line":1,"message":MESSAGE},"meta":{}});
    assert_eq!(
        document,
        json!({"code":1,"stdout":"","stderr":format!("Error: {catch_up}\n")})
    );
    assert_eq!(
        answer(&repo, &[RULE, "--view", "review"]),
        json!({"code":1,"stdout":"","stderr":format!("Error: {warning}\n")})
    );
    let etag = clean.0["edit"]["etag"].as_str().unwrap();
    assert_update_refused(&server, &repo, etag, &refused, &refused).await;
    server.close().await;
}

#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_get_over_a_withdrawal_that_names_no_submission_with_no_stored_revision_refuses() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    write_lost_withdrawal(directory.path());
    std::fs::remove_dir_all(directory.path().join(".provenance/cache")).unwrap();
    let server = Server::start(&repo).await;

    let (code, stdout, stderr) = run(&repo, &[RULE, "get", "--format", "json"]);
    assert_eq!((code, stdout.as_str()), (Some(1), ""), "{stderr}");
    let by_cli = crate::porcelain_api::envelope(&stderr);
    let message = by_cli["error"]["message"].as_str().unwrap();
    let cause = format!("catch-up failed: {WITHDRAWALS} line 1: {MESSAGE}; {REPAIR}");
    let database = message
        .strip_prefix("no projection revision in ")
        .and_then(|rest| rest.strip_suffix(&format!("; run `provenance materialize` ({cause})")))
        .unwrap_or_else(|| panic!("{message}"));
    assert!(database.ends_with("provenance.db"), "{message}");
    assert_eq!(
        by_cli,
        json!({"error":{"kind":"operation_failed","message":message},"meta":{}})
    );
    let called = server.client.call("get", json!({"target":RULE})).await;
    assert_eq!(called["isError"], json!(true), "{called}");
    let by_mcp: Value =
        serde_json::from_str(called["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(by_mcp, by_cli);
    server.close().await;

    assert_eq!(
        answer(&repo, &["materialize"]),
        json!({"code":1,"stdout":"","stderr":format!("Error: {WITHDRAWALS} line 1: {MESSAGE}; {REPAIR}\n")})
    );
}
