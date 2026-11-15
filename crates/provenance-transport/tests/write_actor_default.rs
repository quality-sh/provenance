//! A Requirement create, a Requirement update and a Discussion start through the HTTP API
//! that give no actor record the actor `authoring`. A Discussion status change still needs
//! its actor.
#![cfg(feature = "test-fixture")]
use crate::support;
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, call_with_headers, host};

const PARENT: &str = "/requirements/req_shared/discussions";

/// The data of one successful read.
async fn read(host: &provenance_transport::StatementHost, path: &str) -> Value {
    let (status, read) = call(host, "GET", path, None).await;
    assert_eq!(status, 200, "{read}");
    read["data"].clone()
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_api_requirement_create_with_no_actor_records_authoring() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let body = json!({"data":{"id":"req_new", "statement":"The system stores each record.",
        "status":"active", "depends_on":[], "supersedes":[]}});
    let (status, created) = call(&host, "POST", "/requirements", Some(body)).await;
    assert_eq!(status, 200, "{created}");
    let stored = read(&host, "/requirements/req_new").await;
    assert_eq!(stored["decision"]["pending"]["actor"], "authoring");
    assert_eq!(
        (&stored["id"], &stored["statement"], &stored["status"]),
        (
            &json!("req_new"),
            &json!("The system stores each record."),
            &json!("active")
        )
    );
    assert_eq!(created, json!({"data": stored, "meta": {}}));
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_api_requirement_update_with_no_actor_records_authoring() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let etag = read(&host, "/requirements/req_shared").await["edit"]["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    let body = json!({"data":{"description":"Edited."}});
    let (status, updated) = call_with_headers(
        &host,
        "PATCH",
        "/requirements/req_shared",
        Some(body),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{updated}");
    let stored = read(&host, "/requirements/req_shared").await;
    assert_eq!(stored["decision"]["pending"]["actor"], "authoring");
    assert_eq!(stored["description"], "Edited.");
    assert_eq!(updated, json!({"data": stored, "meta": {}}));
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_api_discussion_start_with_no_actor_records_authoring() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let body = json!({"data":{"declared_by":null, "role":"user", "body":"A concern."}});
    let (status, started) = call(&host, "POST", PARENT, Some(body)).await;
    assert_eq!(status, 200, "{started}");
    let id = started["data"]["discussion_id"].as_str().unwrap();
    let stored = read(&host, &format!("{PARENT}/{id}")).await["discussion"].clone();
    let root = stored["root_message_id"].clone();
    assert_eq!(
        stored,
        json!({
            "scope_id": "default", "discussion_id": id,
            "parent": {"node_type": "requirement", "node_id": "req_shared"},
            "thread_id": "thread_requirement_req_shared", "root_message_id": root,
            "message_ids": [root], "status": "active", "version": 1, "actor": "authoring",
            "outcomes": [], "disposition_id": null,
        })
    );
    assert_eq!(started, json!({"data": stored, "meta": {}}));
}

/// A status change names the actor who makes it, so a status change with no actor is
/// refused and the Discussion stays as it is.
#[tokio::test]
#[verifies("rule_write_actor_required", examples)]
async fn an_api_discussion_status_change_with_no_actor_is_refused() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let body = json!({"data":{"actor":"reviewer", "declared_by":null, "role":"user",
        "body":"A concern."}});
    let (status, started) = call(&host, "POST", PARENT, Some(body)).await;
    assert_eq!(status, 200, "{started}");
    let discussion = format!(
        "{PARENT}/{}",
        started["data"]["discussion_id"].as_str().unwrap()
    );
    let body = json!({"data":{"declared_by":null, "status":"resolved"}});
    let (status, refused) = call_with_headers(
        &host,
        "PATCH",
        &discussion,
        Some(body),
        &[("if-match", "\"1\"")],
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(
        refused,
        json!({"error":{"kind":"invalid_input", "field":"request.actor", "reason":"required"},
            "meta":{}})
    );
    let stored = read(&host, &discussion).await["discussion"].clone();
    assert_eq!(
        (&stored["status"], &stored["version"]),
        (&json!("active"), &json!(1))
    );
}
