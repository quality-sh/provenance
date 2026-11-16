//! A Withdrawal row that names no review submission makes the review rows of
//! the scope disagree, and the cache catch-up fails with it. When the read
//! database holds a stored revision, the get and the list read answer at that
//! serial with a warning that names the file and the line of the row, on the
//! CLI and on a spawned `provenance mcp`. The api read of the record, the
//! review view and an update refuse with that row, and the document read of a
//! Requirement refuses with the failed catch-up. When the read database holds
//! no revision, the get and `provenance materialize` refuse. The Withdrawal
//! fault wins over a fault of a scope that the catch-up validates after it,
//! and over a Discussion fault of its own scope.

use provenance_macros::verifies;
use serde_json::{json, Value};
use tempfile::TempDir;

use crate::cli_get_damaged_state::{
    assert_no_revision_refusal, assert_stored_record, assert_update_refused, clean_get,
    cli_refusal, mcp_refusal, run,
};
use crate::cli_mcp_read_parity::{chain, cli_json};
use crate::cli_mcp_record_actions::Server;

const RULE: &str = "rule_overtime_pay";
const WITHDRAWALS: &str = "scopes/default/ideation/withdrawals.jsonl";
const MESSAGE: &str = "withdrawal withdrawal_lost names no review submission: proposal_missing";
const REPAIR: &str = "To repair, remove ideation/withdrawals.jsonl line 1, then commit.";

/// The text of the refusal of the Withdrawal row.
fn lost_withdrawal() -> String {
    format!("{WITHDRAWALS} line 1: {MESSAGE}; {REPAIR}")
}

/// Writes `line` as the only line of the state file `file`.
fn write_state_line(root: &std::path::Path, file: &str, line: &str) {
    let path = root.join(".provenance/state").join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("{line}\n")).unwrap();
}

/// Writes a Withdrawal that names no review submission as line 1 of the
/// Withdrawal file.
fn write_lost_withdrawal(root: &std::path::Path) {
    let withdrawal = json!({"scope_id":"default","id":"withdrawal_lost",
        "proposal_id":"proposal_missing","actor":"reviewer","reason":null});
    write_state_line(root, WITHDRAWALS, &withdrawal.to_string());
}

/// A repository with the records of `chain`, whose read database holds a
/// stored revision, and the JSON and the text of the get of the Rule at that
/// revision.
fn stored() -> (TempDir, String, (Value, String)) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let clean = clean_get(&repo);
    (directory, repo, clean)
}

/// The JSON get of the Rule answers with `clean` at the stored serial, with
/// the policy word `catch_up_failed` and the freshness error `warning`.
fn assert_get_warns(repo: &str, clean: &(Value, String), warning: &str) {
    let mut expected = clean.0.clone();
    expected["record_metadata"]["freshness_error"] = json!(warning);
    expected["record_metadata"]["stamp"]["policy"] = json!("catch_up_failed");
    assert_eq!(cli_json(repo, &[RULE, "get"]), expected);
}

/// The exit code, the standard output and the standard error of one CLI
/// command, with each path in the standard error written with `/`.
fn answer(repo: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run(repo, args);
    json!({"code": code, "stdout": stdout, "stderr": stderr.replace('\\', "/")})
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn the_get_and_the_list_read_warn_at_the_stored_serial() {
    let (directory, repo, clean) = stored();
    let listed = cli_json(&repo, &["api", "rules"]);
    write_lost_withdrawal(directory.path());
    let server = Server::start(&repo).await;

    assert_stored_record(&server, &repo, &clean, &lost_withdrawal()).await;
    server.close().await;
    let mut expected = listed;
    expected["meta"]["freshness_error"] = json!(lost_withdrawal());
    expected["meta"]["stamp"]["policy"] = json!("catch_up_failed");
    assert_eq!(cli_json(&repo, &["api", "rules"]), expected);
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn the_api_read_the_review_view_and_an_update_refuse_with_the_line() {
    let (directory, repo, clean) = stored();
    write_lost_withdrawal(directory.path());
    let server = Server::start(&repo).await;

    let refused = json!({"error":{"file":WITHDRAWALS,"hint":REPAIR,"kind":"invalid_state_file",
        "line":1,"message":MESSAGE},"meta":{}});
    assert_eq!(
        cli_refusal(&repo, &["api", &format!("rules/{RULE}")]),
        refused
    );
    assert_eq!(
        answer(&repo, &[RULE, "--view", "review"]),
        json!({"code":1,"stdout":"","stderr":format!("Error: {}\n", lost_withdrawal())})
    );
    let etag = clean.0["edit"]["etag"].as_str().unwrap();
    assert_update_refused(&server, &repo, etag, &refused, &refused).await;
    server.close().await;
}

#[test]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
fn the_document_read_refuses_with_the_failed_catch_up() {
    let (directory, repo, _) = stored();
    write_lost_withdrawal(directory.path());

    assert_eq!(
        cli_refusal(&repo, &["api", "requirements/req_overtime/document"]),
        json!({"error":{"file":WITHDRAWALS,"hint":REPAIR,"kind":"document_catch_up_failed",
            "line":1,"message":MESSAGE},"meta":{}})
    );
}

#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn the_get_with_no_stored_revision_refuses() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    write_lost_withdrawal(directory.path());
    std::fs::remove_dir_all(directory.path().join(".provenance/cache")).unwrap();
    let server = Server::start(&repo).await;

    let by_cli = cli_refusal(&repo, &[RULE, "get"]);
    assert_no_revision_refusal(&by_cli, &lost_withdrawal());
    assert_eq!(
        mcp_refusal(&server, "get", json!({"target":RULE})).await,
        by_cli
    );
    server.close().await;

    // No Rule states the answer of `provenance materialize`. This shows that the
    // command that the refusal names gives the row and its repair.
    assert_eq!(
        answer(&repo, &["materialize"]),
        json!({"code":1,"stdout":"","stderr":format!("Error: {}\n", lost_withdrawal())})
    );
}

#[test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
fn a_withdrawal_fault_wins_over_an_ideation_fault_of_a_scope_that_is_validated_after_it() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let manifest = directory.path().join(".provenance/state/manifest.json");
    let mut declared: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    declared["scopes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"later","path_prefix":"later"}));
    std::fs::write(&manifest, format!("{declared}\n")).unwrap();
    let clean = clean_get(&repo);
    write_lost_withdrawal(directory.path());
    let cards = "scopes/later/ideation/proposal_cards.jsonl";
    write_state_line(directory.path(), cards, r#"{"scope_id":"later","#);

    assert_get_warns(&repo, &clean, &lost_withdrawal());
}

#[test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
fn a_withdrawal_fault_wins_over_a_discussion_that_names_a_missing_thread() {
    let (directory, repo, clean) = stored();
    write_lost_withdrawal(directory.path());
    let discussion = json!({"scope_id":"default","discussion_id":"discussion_lost",
        "parent":{"node_type":"requirement","node_id":"req_overtime"},
        "thread_id":"thread_missing","root_message_id":"message_root",
        "message_ids":["message_root"],"status":"active","version":1,"actor":"reviewer",
        "outcomes":[],"disposition_id":null});
    let discussions = "scopes/default/threads/discussions.jsonl";
    write_state_line(directory.path(), discussions, &discussion.to_string());

    assert_get_warns(&repo, &clean, &lost_withdrawal());
}
