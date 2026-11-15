//! A get of a record over state that does not parse gives the same result on the CLI and
//! on a spawned `provenance mcp`. When the read database holds a stored revision, the get
//! gives the record at that serial with its edit field and the freshness warning. When it
//! holds no revision, the get refuses and names `provenance materialize`.

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::io::Write as _;
use std::path::Path;

use crate::cli_mcp_read_parity::{chain, cli, cli_json, mcp};
use crate::cli_mcp_record_actions::Server;

const RULE: &str = "rule_overtime_pay";
const RULES: &str = "scopes/default/rules/rule.jsonl";
const CARDS: &str = "scopes/default/ideation/proposal_cards.jsonl";

/// Appends a line that does not parse to the state file `file`, and gives its line number.
fn damage(root: &Path, file: &str) -> usize {
    let path = root.join(".provenance/state").join(file);
    let lines = std::fs::read_to_string(&path).unwrap().lines().count();
    let mut opened = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    opened.write_all(b"{\"scope_id\":\"default\",\n").unwrap();
    lines + 1
}

/// The exit code, the standard output and the standard error of one CLI command.
fn run(repo: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
    (
        output.status.code(),
        text(output.stdout),
        text(output.stderr),
    )
}

/// The refusal object of one CLI command that exits with code 1.
fn cli_refusal(repo: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run(repo, &[args, &["--format", "json"][..]].concat());
    assert_eq!((code, stdout.as_str()), (Some(1), ""), "{args:?}: {stderr}");
    crate::porcelain_api::envelope(&stderr)
}

/// The refusal object of one MCP tool call that fails.
async fn mcp_refusal(server: &Server, tool: &str, arguments: Value) -> Value {
    let called = server.client.call(tool, arguments).await;
    assert_eq!(called["isError"], json!(true), "{called}");
    serde_json::from_str(called["content"][0]["text"].as_str().unwrap()).unwrap()
}

/// The get of the Rule over damaged state, on each interface: the record at the stored
/// serial with the freshness warning `warning`, and otherwise the answer of `clean`, the
/// JSON and text of the get before the damage.
async fn assert_stored_record(server: &Server, repo: &str, clean: &(Value, String), warning: &str) {
    let by_cli = cli_json(repo, &[RULE, "get"]);
    let mut expected = clean.0.clone();
    expected["record_metadata"]["freshness_error"] = json!(warning);
    expected["record_metadata"]["stamp"]["policy"] = json!("catch_up_failed");
    assert_eq!(by_cli, expected);
    assert_eq!(
        mcp(&server.client, "get", json!({"target":RULE})).await,
        by_cli
    );

    let text = format!("{}\nwarning: record freshness: {warning}\n", clean.1);
    assert_eq!(
        run(repo, &[RULE, "get"]),
        (Some(0), text.clone(), String::new())
    );
    let called = server.client.call("get", json!({"target":RULE})).await;
    assert_eq!(
        called["content"][0]["text"].as_str(),
        text.strip_suffix('\n')
    );

    let link = cli_json(repo, &[RULE, "--review-link", "--no-open"]);
    let message = link["message"].as_str().unwrap();
    assert_eq!(link, json!({"review_url":null,"message":message}));
    let linked = mcp(
        &server.client,
        "get",
        json!({"target":RULE,"review_link":true}),
    )
    .await;
    assert_eq!(linked["review_link"], link);
    assert_eq!(linked["edit"], clean.0["edit"]);
}

/// The JSON and the text of the get of the Rule.
fn clean_get(repo: &str) -> (Value, String) {
    let text = cli(&[RULE, "get", "--repo", repo]);
    (cli_json(repo, &[RULE, "get"]), text)
}

#[tokio::test]
#[verifies("rule_get_record_gives_edit_etag", examples)]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_get_over_a_bad_record_line_gives_the_stored_record_on_each_interface() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let clean = clean_get(&repo);
    let line = damage(directory.path(), RULES);
    let server = Server::start(&repo).await;

    let by_cli = cli_json(&repo, &[RULE, "get"]);
    let warning = by_cli["record_metadata"]["freshness_error"]
        .as_str()
        .unwrap();
    let parse = "EOF while parsing a value at line 1 column 22";
    assert!(warning.starts_with("cannot hash "), "{warning}");
    assert!(
        warning.ends_with(&format!("rule.jsonl: {parse}: {parse}")),
        "{warning}"
    );
    assert_stored_record(&server, &repo, &clean, warning).await;

    let etag = clean.0["edit"]["etag"].as_str().unwrap();
    let refused = json!({"error":{"file":RULES,"kind":"invalid_state_file","line":line,
        "message":"EOF while parsing a value"},"meta":{}});
    let flags = [
        RULE,
        "update",
        "--if-match",
        etag,
        "--description",
        "Changed.",
    ];
    assert_eq!(cli_refusal(&repo, &flags), refused);
    let arguments = json!({"action":"update","target":RULE,"if_match":etag,
        "description":"Changed."});
    assert_eq!(mcp_refusal(&server, "change", arguments).await, refused);
    server.close().await;
}

#[tokio::test]
#[verifies("rule_get_record_gives_edit_etag", examples)]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_get_over_a_bad_review_history_line_gives_the_stored_record_on_each_interface() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let clean = clean_get(&repo);
    let line = damage(directory.path(), CARDS);
    let server = Server::start(&repo).await;

    let warning = format!("{CARDS} line {line}: EOF while parsing a value");
    assert_stored_record(&server, &repo, &clean, &warning).await;
    server.close().await;
}

#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_get_over_a_bad_line_with_no_stored_revision_refuses_on_each_interface() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let line = damage(directory.path(), RULES);
    std::fs::remove_dir_all(directory.path().join(".provenance/cache")).unwrap();
    let server = Server::start(&repo).await;

    let by_cli = cli_refusal(&repo, &[RULE, "get"]);
    let message = by_cli["error"]["message"].as_str().unwrap();
    let cause = format!("catch-up failed: {RULES} line {line}: EOF while parsing a value");
    let database = message
        .strip_prefix("no projection revision in ")
        .and_then(|rest| rest.strip_suffix(&format!("; run `provenance materialize` ({cause})")))
        .unwrap_or_else(|| panic!("{message}"));
    assert!(database.ends_with("provenance.db"), "{message}");
    let refused = json!({"error":{"kind":"operation_failed","message":message},"meta":{}});
    assert_eq!(by_cli, refused);
    assert_eq!(
        mcp_refusal(&server, "get", json!({"target":RULE})).await,
        refused
    );
    let flags = [RULE, "--review-link", "--no-open"];
    assert_eq!(cli_refusal(&repo, &flags), refused);
    let arguments = json!({"target":RULE,"review_link":true});
    assert_eq!(mcp_refusal(&server, "get", arguments).await, refused);
    server.close().await;
}
