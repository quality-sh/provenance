//! Characterization: the kind, status and fields of each refusal that the HTTP
//! routes give on the base commit. Later changes to the store refusals keep
//! these rows unchanged.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, call_with_headers, host};

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

fn requirement(id: &str, extra: &Value) -> Value {
    let mut data = json!({
        "actor":"reviewer", "id":id, "statement":"The system stores each record.",
        "status":"discovery", "depends_on":[], "supersedes":[]
    });
    for (key, value) in extra.as_object().unwrap() {
        data[key] = value.clone();
    }
    json!({ "data": data })
}

async fn etag(host: &provenance_transport::StatementHost, id: &str) -> String {
    let (status, read) = call(host, "GET", &format!("/requirements/{id}"), None).await;
    assert_eq!(status, 200, "{read}");
    read["data"]["edit"]["etag"].as_str().unwrap().to_owned()
}

/// One refusal row: its name, the status and the `error` object.
fn row(name: &str, (status, mut body): (u16, Value)) -> Value {
    if body["error"]["current_etag"].is_string() {
        body["error"]["current_etag"] = json!("<etag>");
    }
    json!({"row": name, "status": status, "error": body["error"]})
}

async fn write_rows(repo: &Repository) -> Vec<Value> {
    let host = host(repo, true);
    let mut rows = Vec::new();
    rows.push(row(
        "create an existing requirement",
        call(
            &host,
            "POST",
            "/requirements",
            Some(requirement("req_shared", &json!({}))),
        )
        .await,
    ));
    rows.push(row(
        "create with a missing dependency",
        call(
            &host,
            "POST",
            "/requirements",
            Some(requirement(
                "req_new",
                &json!({"depends_on":["req_missing"]}),
            )),
        )
        .await,
    ));
    rows.push(row(
        "create with a blank actor",
        call(
            &host,
            "POST",
            "/requirements",
            Some(requirement("req_new", &json!({"actor":" "}))),
        )
        .await,
    ));
    rows.push(row(
        "update with an old etag",
        call_with_headers(
            &host,
            "PATCH",
            "/requirements/req_shared",
            Some(json!({"data":{"actor":"reviewer","description":"Edited."}})),
            &[("if-match", "\"stale\"")],
        )
        .await,
    ));
    let (status, created) = call(
        &host,
        "POST",
        "/requirements",
        Some(requirement("req_old", &json!({}))),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let guard = etag(&host, "req_old").await;
    let (status, archived) = call_with_headers(
        &host,
        "PATCH",
        "/requirements/req_old",
        Some(json!({"data":{"actor":"reviewer","status":"archived",
            "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}})),
        &[("if-match", &guard)],
    )
    .await;
    assert_eq!(status, 200, "{archived}");
    rows.push(row(
        "create under an archived parent",
        call(
            &host,
            "POST",
            "/requirements",
            Some(requirement("req_child", &json!({"refines":["req_old"]}))),
        )
        .await,
    ));
    rows.extend(discussion_rows(&host).await);
    rows
}

async fn discussion_rows(host: &provenance_transport::StatementHost) -> Vec<Value> {
    let mut rows = Vec::new();
    let parent = "/requirements/req_shared/discussions";
    rows.push(row(
        "start a discussion with an empty body",
        call(
            host,
            "POST",
            parent,
            Some(json!({"data":{"actor":"reviewer","declared_by":null,"role":"user","body":" "}})),
        )
        .await,
    ));
    let (status, started) = call(
        host,
        "POST",
        parent,
        Some(json!({"data":{"actor":"reviewer","declared_by":null,"role":"user","body":"A concern."}})),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let id = started["data"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    rows.push(row(
        "reply with an old discussion version",
        call_with_headers(
            host,
            "POST",
            &format!("{parent}/{id}/messages"),
            Some(json!({"data":{"actor":"agent","declared_by":null,"role":"assistant","body":"Reply."}})),
            &[("if-match", "\"9\"")],
        )
        .await,
    ));
    rows.push(row(
        "reply to a missing discussion",
        call_with_headers(
            host,
            "POST",
            &format!("{parent}/thread_missing/messages"),
            Some(json!({"data":{"actor":"agent","declared_by":null,"role":"assistant","body":"Reply."}})),
            &[("if-match", "\"1\"")],
        )
        .await,
    ));
    rows
}

async fn read_rows(repo: &Repository) -> Vec<Value> {
    let host = host(repo, false);
    vec![
        row(
            "read a missing requirement",
            call(&host, "GET", "/requirements/req_missing", None).await,
        ),
        row(
            "read a missing history entry",
            call(
                &host,
                "GET",
                "/requirements/req_shared/history/entry_missing",
                None,
            )
            .await,
        ),
        row(
            "write on a read-only host",
            call(
                &host,
                "POST",
                "/requirements",
                Some(requirement("req_new", &json!({}))),
            )
            .await,
        ),
    ]
}

/// Adds a line that is not a record to the Requirement shard.
async fn state_rows(repo: &Repository) -> Vec<Value> {
    let path = repo.dir.path().join(REQUIREMENTS);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"not\":\"a record\"}\n");
    std::fs::write(&path, text).unwrap();
    let host = host(repo, true);
    vec![
        row(
            "read a shard with a bad line",
            call(&host, "GET", "/requirements/req_shared", None).await,
        ),
        row(
            "write to a shard with a bad line",
            call(
                &host,
                "POST",
                "/requirements",
                Some(requirement("req_new", &json!({}))),
            )
            .await,
        ),
    ]
}

/// An update of a Requirement that does not exist is `resource_not_found`;
/// the base commit gave 500 `write_failed`.
#[tokio::test]
#[provenance_macros::verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn an_update_of_a_missing_requirement_is_not_found() {
    let repo = Repository::new("The shared graph is readable.");
    let answer = call_with_headers(
        &host(&repo, true),
        "PATCH",
        "/requirements/req_missing",
        Some(json!({"data":{"actor":"reviewer","description":"Edited."}})),
        &[("if-match", "\"stale\"")],
    )
    .await;
    assert_eq!(
        row("update a missing requirement", answer),
        json!({"error":{"kind":"resource_not_found"},"row":"update a missing requirement","status":404})
    );
}

/// Review refusals that the base commit gave as 500 `write_failed`.
#[tokio::test]
#[provenance_macros::verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn a_review_refusal_has_a_declared_kind() {
    let repo = Repository::new("The shared graph is readable.");
    support::records::allow_reviewer(&repo);
    let host = host(&repo, true);
    let missing = "/requirements/req_shared/submissions/proposal_missing";
    let mut rows = Vec::new();
    for (name, path, data) in [
        (
            "withdraw with a blank reason",
            format!("{missing}/withdraw"),
            json!({"actor":"agent", "declared_by":null, "reason":" "}),
        ),
        (
            "withdraw a missing submission",
            format!("{missing}/withdraw"),
            json!({"actor":"agent", "declared_by":null, "reason":"Revise it."}),
        ),
        (
            "decide a missing submission",
            format!("{missing}/decide"),
            json!({"actor":{"identity_type":"human","id":"reviewer"},
                "decision":"rejected", "rationale":"Name the scope.",
                "canonical_artifact":null,
                "feedback":{"role":"user","body":"Name the scope."},
                "declared_by":null}),
        ),
    ] {
        rows.push(row(
            name,
            call(&host, "POST", &path, Some(json!({ "data": data }))).await,
        ));
    }
    assert_eq!(
        Value::Array(rows),
        json!([
            {"error":{"field":"request.reason","kind":"invalid_input","reason":"invalid_value"},"row":"withdraw with a blank reason","status":400},
            {"error":{"kind":"resource_not_found"},"row":"withdraw a missing submission","status":404},
            {"error":{"kind":"resource_not_found"},"row":"decide a missing submission","status":404}
        ])
    );
}

fn proposal(id: &str, extra: &Value) -> Value {
    let mut data = json!({
        "scope_id":"default", "id":id, "proposal_key":"shared", "proposal_type":"requirement_candidate",
        "title":"Shared", "summary":"Clarify the shared requirement.", "confidence":null,
        "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_shared"},
            "source_ids":[], "evidence_references":[], "supporting_claim_ids":[]},
        "builds_on":[], "promotion_state":"proposed", "duplicate_of":null, "superseded_by":null,
        "record_revision":null, "actor":null, "revises":null, "revises_rejection":null
    });
    for (key, value) in extra.as_object().unwrap() {
        data[key] = value.clone();
    }
    json!({ "data": data })
}

/// Proposal refusals that the base commit gave as 500 `write_failed`.
#[tokio::test]
#[provenance_macros::verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn a_proposal_refusal_has_a_declared_kind() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let (status, created) = call(
        &host,
        "POST",
        "/proposals",
        Some(proposal("proposal_shared", &json!({}))),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let mut rows = Vec::new();
    for (name, body) in [
        (
            "create an existing proposal",
            proposal("proposal_shared", &json!({})),
        ),
        (
            "create a duplicate proposal with no duplicate_of",
            proposal("proposal_twin", &json!({"promotion_state":"duplicate"})),
        ),
    ] {
        rows.push(row(
            name,
            call(&host, "POST", "/proposals", Some(body)).await,
        ));
    }
    assert_eq!(
        Value::Array(rows),
        json!([
            {"error":{"kind":"already_exists"},"row":"create an existing proposal","status":409},
            {"error":{"field":"request.duplicate_of","kind":"invalid_input","reason":"invalid_value"},"row":"create a duplicate proposal with no duplicate_of","status":400}
        ])
    );
}

#[tokio::test]
async fn each_refusal_keeps_its_kind_status_and_fields() {
    let repo = Repository::new("The shared graph is readable.");
    let mut rows = write_rows(&repo).await;
    rows.extend(read_rows(&repo).await);
    rows.extend(state_rows(&repo).await);
    assert_eq!(Value::Array(rows), expected_rows());
}

/// The rows on the base commit.
fn expected_rows() -> Value {
    json!([
        {"error":{"kind":"already_exists"},"row":"create an existing requirement","status":409},
        {"error":{"kind":"missing_reference"},"row":"create with a missing dependency","status":400},
        {"error":{"field":"request.actor","kind":"invalid_input","reason":"invalid_value"},"row":"create with a blank actor","status":400},
        {"error":{"current_etag":"<etag>","kind":"requirement_edit_conflict"},"row":"update with an old etag","status":409},
        {"error":{"kind":"invalid_update","message":"Requirement req_old is archived; a new link cannot point to it"},"row":"create under an archived parent","status":400},
        {"error":{"kind":"empty_message_body"},"row":"start a discussion with an empty body","status":400},
        {"error":{"kind":"discussion_version_conflict"},"row":"reply with an old discussion version","status":409},
        {"error":{"kind":"resource_not_found"},"row":"reply to a missing discussion","status":404},
        {"error":{"kind":"resource_not_found"},"row":"read a missing requirement","status":404},
        {"error":{"kind":"resource_not_found"},"row":"read a missing history entry","status":404},
        {"error":{"kind":"access_denied"},"row":"write on a read-only host","status":403},
        {"error":{"file":"scopes/default/requirements/req.jsonl","kind":"invalid_state_file","line":3,"message":"missing field `scope_id`"},"row":"read a shard with a bad line","status":500},
        {"error":{"file":"scopes/default/requirements/req.jsonl","kind":"invalid_state_file","line":3,"message":"missing field `scope_id`"},"row":"write to a shard with a bad line","status":500}
    ])
}
