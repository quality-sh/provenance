//! A get of a record gives the edit ETag of the record at `edit.etag` on the CLI and on
//! a spawned `provenance mcp`, for each record kind. The ETag is the one that the `api`
//! read of the record gives, and it guards the next update on each interface. The other
//! views have no `edit` field, and the review view keeps its ETag at `review.edit`.

use provenance_macros::verifies;
use serde_json::{json, Value};

use crate::cli_mcp_read_parity::{chain, cli, cli_json, mcp};
use crate::cli_mcp_record_actions::Server;

/// One record of each kind, the `api` collection of its kind, and one field that an
/// update changes.
const RECORDS: [(&str, &str, &str); 6] = [
    ("source_award", "sources", "name"),
    ("req_overtime", "requirements", "description"),
    ("rule_overtime_pay", "rules", "description"),
    ("res_overtime", "resolutions", "title"),
    ("topic_overtime", "topics", "title"),
    ("q_overtime", "questions", "question"),
];

/// The `api` read of one record.
fn api_read(repo: &str, id: &str, family: &str) -> Value {
    cli_json(repo, &["api", &format!("{family}/{id}")])
}

/// The keys of the CLI JSON record view.
const RECORD_VIEW_KEYS: [&str; 8] = [
    "bounds",
    "detail",
    "edit",
    "record",
    "record_metadata",
    "related",
    "review",
    "view",
];

/// The records of [`RECORDS`]: the chain of `cli_mcp_read_parity`, and a Resolution, a
/// Topic, and a Question for its Requirement.
fn records(repo: &str) {
    chain(repo);
    for args in [
        &[
            "res_overtime",
            "create",
            "--type",
            "resolution",
            "--requirement-id",
            "req_overtime",
            "--title",
            "Overtime threshold",
            "--position",
            "Use the award threshold.",
            "--rationale",
            "The award sets the threshold.",
        ][..],
        &[
            "topic_overtime",
            "create",
            "--type",
            "topic",
            "--requirement-id",
            "req_overtime",
            "--title",
            "Overtime",
        ],
        &[
            "q_overtime",
            "create",
            "--type",
            "question",
            "--topic-id",
            "topic_overtime",
            "--question",
            "Which threshold applies?",
            "--resolution-method",
            "research",
        ],
    ] {
        cli_json(repo, args);
    }
}

/// The `edit.etag` of one get result.
fn etag(read: &Value) -> String {
    read["edit"]["etag"]
        .as_str()
        .unwrap_or_else(|| panic!("no edit.etag: {read}"))
        .to_owned()
}

/// The keys of one JSON object, in order.
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[tokio::test]
#[verifies("rule_get_record_gives_edit_etag", examples)]
async fn a_get_of_each_record_gives_the_same_edit_etag_on_the_cli_and_on_mcp() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    records(&repo);
    let server = Server::start(&repo).await;

    for (id, family, _) in RECORDS {
        let by_cli = cli_json(&repo, &[id, "get"]);
        let by_mcp = mcp(&server.client, "get", json!({"target":id})).await;
        assert_eq!(by_mcp, by_cli, "{id}");
        assert_eq!(keys(&by_cli), RECORD_VIEW_KEYS, "{id}");
        let member = api_read(&repo, id, family);
        assert_eq!(by_cli["edit"], member["data"]["edit"], "{id}");

        let line = format!("\n\nedit.etag: {}", etag(&by_cli));
        let text = cli(&[id, "get", "--repo", &repo]);
        assert!(text.ends_with(&format!("{line}\n")), "{id}: {text}");
        let called = server.client.call("get", json!({"target":id})).await;
        let called = called["content"][0]["text"].as_str();
        assert_eq!(called, text.strip_suffix('\n'), "{id}");
    }
    server.close().await;
}

/// The update arguments of `change` for one record of the `api` collection `family`: the
/// field and its new text, the ETag, and the actor that a Requirement update needs on MCP.
fn change_update(id: &str, family: &str, field: &str, etag: &str) -> Value {
    let mut arguments = json!({"action":"update","target":id,"if_match":etag});
    arguments[field] = json!("Changed on MCP.");
    if family == "requirements" {
        arguments["actor"] = json!("agent");
    }
    arguments
}

#[tokio::test]
#[verifies("rule_get_record_gives_edit_etag", examples)]
async fn the_edit_etag_of_a_get_guards_the_next_update_on_each_interface() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    records(&repo);
    let server = Server::start(&repo).await;

    for (id, family, field) in RECORDS {
        let read = mcp(&server.client, "get", json!({"target":id})).await;
        let arguments = change_update(id, family, field, &etag(&read));
        let updated = server.client.call("change", arguments).await;
        assert_ne!(updated["isError"], json!(true), "{id}: {updated}");

        let read = cli_json(&repo, &[id, "get"]);
        let flag = format!("--{field}");
        let text = "Changed on the CLI.";
        cli_json(
            &repo,
            &[id, "update", "--if-match", &etag(&read), &flag, text],
        );
        let member = api_read(&repo, id, family);
        assert_eq!(member["data"][field], text, "{id}");
    }
    server.close().await;
}

#[tokio::test]
#[verifies("rule_get_record_gives_edit_etag", examples)]
async fn the_children_trace_and_impact_views_have_no_edit_field() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    records(&repo);
    let server = Server::start(&repo).await;

    for (id, view) in [
        ("req_overtime", "children"),
        ("rule_overtime_pay", "trace"),
        ("req_overtime", "impact"),
    ] {
        let by_cli = cli_json(&repo, &[id, "get", "--view", view]);
        let arguments = json!({"target":id,"view":view});
        let by_mcp = mcp(&server.client, "get", arguments).await;
        assert_eq!(by_mcp, by_cli, "{id} {view}");
        assert_eq!(by_cli["view"], view);
        assert!(by_cli.get("edit").is_none(), "{id} {view}: {by_cli}");
    }
    server.close().await;
}

/// Characterization of the review view, with no Rule binding: the view keeps its ETag at
/// `review.edit` with the update precondition, and has no top-level `edit` field.
#[tokio::test]
async fn the_review_view_keeps_its_edit_inside_the_review() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    records(&repo);
    let server = Server::start(&repo).await;

    for (id, family, _) in RECORDS {
        let by_cli = cli_json(&repo, &[id, "get", "--view", "review"]);
        let arguments = json!({"target":id,"view":"review"});
        let by_mcp = mcp(&server.client, "get", arguments).await;
        assert_eq!(by_mcp, by_cli, "{id}");
        assert!(by_cli.get("edit").is_none(), "{id}: {by_cli}");
        let member = api_read(&repo, id, family);
        let review = &by_cli["review"];
        assert_eq!(review["edit"], member["data"]["edit"], "{id}");
        let precondition = format!("--if-match {}", etag(&member["data"]));
        assert_eq!(review["update_precondition"], precondition, "{id}");
        assert_eq!(
            keys(review),
            [
                "bounds",
                "decision",
                "discussions",
                "edit",
                "follow_up_commands",
                "response_metadata",
                "update_precondition",
            ],
            "{id}"
        );
    }
    server.close().await;
}

#[tokio::test]
#[verifies("rule_porcelain_cli_prime_teaches_domain", examples)]
#[verifies("rule_porcelain_mcp_guidance_native", examples)]
async fn prime_and_the_mcp_instructions_give_the_edit_etag_to_the_update() {
    let sentence = "Before an update, read the record with get. Give its `edit.etag` unchanged \
        to the update as the if-match value.";
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    cli(&["init", "--path", &repo, "--scope", "default", "--path-prefix", "."]);
    let prime = cli(&["prime"]);
    assert!(prime.contains(sentence), "{prime}");
    let server = Server::start(&repo).await;
    let instructions = server.client.instructions().unwrap();
    assert!(instructions.contains(sentence), "{instructions}");
    server.close().await;
}

/// The refusal object of one CLI command that fails.
fn cli_refusal(repo: &str, args: &[&str]) -> Value {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo, "--format", "json"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{args:?}: {text}");
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap()
}

/// When the read model cannot catch up on the state file of the record, the record view
/// on each interface gives the record that the read model holds, the ETag of that
/// record, and the freshness warning. The review view and an update refuse.
#[tokio::test]
#[verifies("rule_get_record_gives_edit_etag", examples)]
async fn a_record_with_an_unreadable_state_file_gives_the_etag_of_the_record_that_get_shows() {
    use std::io::Write as _;

    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    chain(&repo);
    let before = cli_json(&repo, &["rule_overtime_pay", "get"]);
    let rules = directory
        .path()
        .join(".provenance/state/scopes/default/rules/rule.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(rules)
        .unwrap();
    file.write_all(b"{\"scope_id\":\"default\",\n").unwrap();
    let server = Server::start(&repo).await;

    let by_cli = cli_json(&repo, &["rule_overtime_pay", "get"]);
    let arguments = json!({"target":"rule_overtime_pay"});
    let by_mcp = mcp(&server.client, "get", arguments).await;
    assert_eq!(by_mcp, by_cli);
    assert_eq!(by_cli["edit"], before["edit"]);
    assert!(
        by_cli["record_metadata"]["freshness_error"].is_string(),
        "{by_cli}"
    );

    let review = cli_refusal(&repo, &["rule_overtime_pay", "get", "--view", "review"]);
    assert_eq!(review["error"]["kind"], "operation_failed", "{review}");
    let arguments = json!({"target":"rule_overtime_pay","view":"review"});
    let by_mcp = server.client.call("get", arguments).await;
    assert_eq!(by_mcp["isError"], json!(true), "{by_mcp}");
    let text = by_mcp["content"][0]["text"].as_str().unwrap();
    assert_eq!(serde_json::from_str::<Value>(text).unwrap(), review);

    let etag = etag(&by_cli);
    let update = [
        "rule_overtime_pay",
        "update",
        "--if-match",
        &etag,
        "--description",
        "Changed.",
    ];
    let refused = cli_refusal(&repo, &update);
    assert_eq!(refused["error"]["kind"], "invalid_state_file", "{refused}");
    server.close().await;
}
