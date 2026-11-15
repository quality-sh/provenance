//! A change of a Rule or a Requirement to active or archived through the change tool
//! opens a status request. The result keeps the route value, and its text names the
//! request.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use support::fixture_host;
use support::mcp::{accept_pending, accept_status_request, call, read, Client, Session};
use support::records::{allow_reviewer, Repository};

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// A repository that allows the reviewer, with the creation submissions of
/// `req_shared` and `rule_shared` accepted, and a session on it.
async fn session() -> (Repository, Session) {
    let repository = Repository::new("The shared graph is readable.");
    allow_reviewer(&repository);
    let session = Session::start(fixture_host::full(&repository)).await;
    accept_pending(&session.client, "requirements", "req_shared").await;
    accept_status_request(&session.client, "rules", "rule_shared").await;
    (repository, session)
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
    let written = &structured["data"];
    assert_eq!(written["status"], "active", "{structured}");
    let (proposal, status) = pending_request(client, "rules", "rule_shared").await;
    assert_eq!(status, "archived");
    let pending = &written["decision"]["pending"];
    assert_eq!(pending["proposal_id"], proposal.as_str(), "{structured}");
    assert_eq!(pending["status_request"]["status"], "archived", "{structured}");
    let readable = text(&result);
    assert!(
        readable.starts_with(&format!(
            "Status request {proposal} is open. The status of rule_shared changes to archived"
        )),
        "{readable}"
    );
    assert!(
        readable.contains(&format!(
            "`provenance api rules/rule_shared/submissions/{proposal}/decide --method post \
             --input -`"
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

/// Creates the draft Rule `rule_new` under `req_shared` with the change tool.
async fn create_rule(client: &Client, status: &str) -> CallToolResult {
    call(
        client,
        "change",
        json!({"action": "create", "target": "rule_new", "type": "rule",
               "statement": "The shared graph is readable.", "status": status,
               "requirement_ids": ["req_shared"], "resolution_ids": [],
               "severity": "high"}),
    )
    .await
}

#[tokio::test]
#[verifies("rule_status_change_waits_for_review", examples)]
#[verifies("rule_status_approval_sets_status", examples)]
async fn a_rule_activation_names_the_status_request_and_keeps_the_status() {
    let (_repository, session) = session().await;
    let client = &session.client;
    let created = create_rule(client, "draft").await;
    assert_ne!(created.is_error, Some(true), "{created:?}");
    accept_pending(client, "rules", "rule_new").await;

    let result = update(client, "rules", "rule_new", json!({"status": "active"})).await;

    assert_ne!(result.is_error, Some(true), "{result:?}");
    let structured = result.structured_content.clone().unwrap();
    let written = &structured["data"];
    assert_eq!(written["status"], "draft", "{structured}");
    let (proposal, status) = pending_request(client, "rules", "rule_new").await;
    assert_eq!(status, "active");
    assert_eq!(written["decision"]["pending"]["proposal_id"], proposal.as_str());
    let readable = text(&result);
    assert!(
        readable.starts_with(&format!(
            "Status request {proposal} is open. The status of rule_new changes to active"
        )),
        "{readable}"
    );
    assert!(
        readable.contains("Until then, rule_new keeps the status draft."),
        "{readable}"
    );

    accept_status_request(client, "rules", "rule_new").await;
    let active = read(client, "rules", "rule_new").await;
    assert_eq!(active["status"], "active", "{active}");
    assert!(active["decision"]["pending"].is_null(), "{active}");
    session.close().await;
}

#[tokio::test]
#[verifies("rule_new_active_rule_waits_for_review", examples)]
async fn a_status_request_with_no_reviewer_names_the_command_that_adds_one() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(fixture_host::full(&repository)).await;
    let client = &session.client;

    let result = create_rule(client, "active").await;

    assert_ne!(result.is_error, Some(true), "{result:?}");
    let structured = result.structured_content.clone().unwrap();
    assert_eq!(structured["data"]["status"], "review", "{structured}");
    let readable = text(&result);
    assert!(readable.starts_with("Status request "), "{readable}");
    assert!(
        readable.contains(
            "The manifest field disposition_actor_ids is empty, thus no actor can make a review \
             decision. To change this field, use the command provenance init \
             --disposition-actor-id <DISPOSITION_ACTOR_ID>."
        ),
        "{readable}"
    );
    session.close().await;
}
