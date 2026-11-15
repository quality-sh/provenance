//! A change of a Rule or a Requirement to active or archived through the change tool
//! opens a status request. The result keeps the route value, and its text names the
//! request.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use support::fixture_host;
use support::mcp::{accept_status_request, call, Client, Session};
use support::records::{allow_reviewer, Repository};

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// A repository that allows the reviewer, with the creation submissions of
/// `req_shared` and `rule_shared` accepted, and a session on it.
async fn session() -> (Repository, Session) {
    let repository = Repository::new("The shared graph is readable.");
    allow_reviewer(&repository);
    let session = Session::start(fixture_host::full(&repository)).await;
    accept_status_request(&session.client, "requirements", "req_shared").await;
    accept_status_request(&session.client, "rules", "rule_shared").await;
    (repository, session)
}

/// Reads one record with its edit and decision state through the `api` tool.
async fn read(client: &Client, collection: &str, id: &str) -> Value {
    let read = call(client, "api", json!({"path": format!("{collection}/{id}")})).await;
    assert_ne!(read.is_error, Some(true), "{read:?}");
    read.structured_content.unwrap()["data"].clone()
}

/// Runs one update of the change tool with the current edit etag.
async fn update(client: &Client, collection: &str, id: &str, fields: Value) -> CallToolResult {
    let etag = read(client, collection, id).await["edit"]["etag"].clone();
    let mut arguments = json!({"action": "update", "target": id, "if_match": etag});
    for (name, value) in fields.as_object().unwrap() {
        arguments[name] = value.clone();
    }
    call(client, "change", arguments).await
}

fn text(result: &CallToolResult) -> String {
    result.content[0].as_text().unwrap().text.clone()
}

/// The pending submission of a record and the status that it requests.
async fn pending_request(client: &Client, collection: &str, id: &str) -> (String, String) {
    let record = read(client, collection, id).await;
    let pending = &record["decision"]["pending"];
    let proposal = pending["proposal_id"].as_str().unwrap().to_owned();
    let status = pending["status_request"]["status"].as_str();
    (
        proposal,
        status.unwrap_or_else(|| panic!("{record}")).to_owned(),
    )
}

#[tokio::test]
#[verifies("rule_status_change_waits_for_review", examples)]
#[verifies("rule_status_approval_sets_status", examples)]
async fn a_rule_archive_names_the_status_request_and_keeps_the_status() {
    let (_repository, session) = session().await;
    let client = &session.client;

    let result = update(
        client,
        "rules",
        "rule_shared",
        json!({"status": "archived", "archived_in_commit": {"commit": COMMIT}}),
    )
    .await;

    assert_ne!(result.is_error, Some(true), "{result:?}");
    let structured = result.structured_content.clone().unwrap();
    let written = structured.get("data").unwrap_or(&structured);
    assert_eq!(written["status"], "active", "{structured}");
    let (proposal, status) = pending_request(client, "rules", "rule_shared").await;
    assert_eq!(status, "archived");
    let readable = text(&result);
    assert!(
        readable.starts_with(&format!(
            "Status request {proposal} is open. The status of rule_shared changes to archived"
        )),
        "{readable}"
    );
    assert!(
        readable.contains("Until then, rule_shared keeps the status active."),
        "{readable}"
    );
    assert!(
        readable.contains("update rule rule_shared\n\n"),
        "{readable}"
    );

    accept_status_request(client, "rules", "rule_shared").await;
    let archived = read(client, "rules", "rule_shared").await;
    assert_eq!(archived["status"], "archived", "{archived}");
    assert!(archived["decision"]["pending"].is_null(), "{archived}");
    session.close().await;
}

#[tokio::test]
#[verifies("rule_status_change_waits_for_review", examples)]
#[verifies("rule_status_approval_sets_status", examples)]
async fn a_requirement_archive_names_the_status_request_and_cascades_on_acceptance() {
    let (_repository, session) = session().await;
    let client = &session.client;

    let result = update(
        client,
        "requirements",
        "req_shared",
        json!({"actor": "agent", "status": "archived",
               "archived_in_commit": {"commit": COMMIT}}),
    )
    .await;

    assert_ne!(result.is_error, Some(true), "{result:?}");
    let structured = result.structured_content.clone().unwrap();
    assert_eq!(structured["data"]["status"], "active", "{structured}");
    let (proposal, status) = pending_request(client, "requirements", "req_shared").await;
    assert_eq!(status, "archived");
    let readable = text(&result);
    assert!(
        readable.starts_with(&format!("Status request {proposal} is open.")),
        "{readable}"
    );
    assert_eq!(
        read(client, "rules", "rule_shared").await["status"],
        "active"
    );

    accept_status_request(client, "requirements", "req_shared").await;
    let requirement = read(client, "requirements", "req_shared").await;
    assert_eq!(requirement["status"], "archived", "{requirement}");
    let rule = read(client, "rules", "rule_shared").await;
    assert_eq!(rule["status"], "archived", "{rule}");
    session.close().await;
}

#[tokio::test]
#[verifies("rule_status_request_changes_one_field", examples)]
async fn a_status_change_with_a_content_change_is_refused() {
    let (_repository, session) = session().await;
    let client = &session.client;

    let result = update(
        client,
        "rules",
        "rule_shared",
        json!({"status": "archived", "archived_in_commit": {"commit": COMMIT},
               "statement": "The shared graph is always readable."}),
    )
    .await;

    assert_eq!(result.is_error, Some(true), "{result:?}");
    let refusal = result.structured_content.clone().unwrap();
    assert_eq!(refusal["error"]["kind"], "invalid_update", "{refusal}");
    let rule = read(client, "rules", "rule_shared").await;
    assert_eq!(rule["status"], "active", "{rule}");
    assert_eq!(rule["statement"], "The shared graph is readable.", "{rule}");
    assert!(rule["decision"]["pending"].is_null(), "{rule}");
    session.close().await;
}

#[tokio::test]
async fn a_status_change_that_needs_no_review_has_no_request_note() {
    let (_repository, session) = session().await;
    let client = &session.client;

    let result = update(
        client,
        "rules",
        "rule_shared",
        json!({"status": "deprecated"}),
    )
    .await;

    assert_ne!(result.is_error, Some(true), "{result:?}");
    let readable = text(&result);
    assert!(
        readable.starts_with("update rule rule_shared\n\n"),
        "{readable}"
    );
    session.close().await;
}
