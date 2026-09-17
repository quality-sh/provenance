#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use axum::{body::Body, http::Request};
use provenance_core::Manifest;
use provenance_store::fixture_probe;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use std::sync::mpsc;
use support::records::Repository;
use tower::ServiceExt as _;

fn host(repo: &Repository) -> StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    StatementHost::with_fixture_access(access)
}

#[allow(clippy::option_if_let_else)]
async fn call(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (u16, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let body = if let Some(value) = body {
        request = request.header("content-type", "application/json");
        Body::from(value.to_string())
    } else {
        Body::empty()
    };
    let response = host
        .router()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value, etag)
}

async fn create(host: &StatementHost, path: &str, key: Option<&str>, data: Value) -> Value {
    let headers = key.map_or_else(Vec::new, |key| vec![("idempotency-key", key)]);
    let (status, value, _) = call(host, "POST", path, Some(json!({"data": data})), &headers).await;
    assert_eq!(status, 200, "{value}");
    value
}

async fn patch(host: &StatementHost, path: &str, data: Value) -> (u16, Value) {
    let (status, value, _) = call(host, "PATCH", path, Some(json!({"data": data})), &[]).await;
    (status, value)
}

#[tokio::test]
async fn patch_refuses_a_non_object_data_value() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);

    for data in [json!(null), json!(false), json!([]), json!("not an object")] {
        let (status, failure, _) = call(
            &host,
            "PATCH",
            "/sources/source_shared",
            Some(json!({"data":data})),
            &[],
        )
        .await;
        assert_eq!(status, 400, "{failure}");
        assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
    }
}

#[tokio::test]
async fn unsupported_methods_use_the_contract_failure_envelope() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    let response = host
        .router()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/sources/source_shared")
                .header("host", "fixture.test")
                .header("authorization", "Bearer fixture-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 405);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(!bytes.is_empty(), "method refusal had no contract envelope");
    let failure: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(failure["error"]["kind"], "method_not_allowed", "{failure}");
    assert!(failure["meta"].is_object(), "{failure}");
}

fn source(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "source_type": "document", "url": null,
        "reference": null, "commit_pin": null, "effective_date": null,
        "review_date": null, "supersedes": [], "origin_thread": null,
        "origin_message": null
    })
}

fn resolution(id: &str, requirement: &str) -> Value {
    json!({
        "id": id, "title": "Decision", "position": "Use the record.",
        "rationale": "The record is available.", "status": "draft",
        "requirement_ids": [requirement], "supersedes": [], "inputs": []
    })
}

async fn enroll(host: &StatementHost) -> String {
    let (status, read, etag) = call(host, "GET", "/requirements/req_shared", None, &[]).await;
    assert_eq!(status, 200, "{read}");
    let etag = etag.unwrap();
    let (status, saved, _) = call(
        host,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent","description":"Enrolled."}})),
        &[
            ("idempotency-key", "enroll_requirement"),
            ("if-match", &etag),
        ],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    saved["data"]["edit"]["revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn submit(host: &StatementHost, proposal: &str, key: &str) {
    let revision = enroll(host).await;
    let body = json!({"data":{
        "actor":"agent", "declared_by":null, "proposal_key":format!("{proposal}-key"),
        "proposal_id":proposal,
        "title":"Review", "summary":"Review the saved Requirement.", "confidence":null,
        "source_ids":[], "evidence_references":[], "builds_on":[],
        "expected_revision":revision, "revises":null
    }});
    let (status, value, _) = call(
        host,
        "POST",
        "/requirements/req_shared/submit",
        Some(body),
        &[("idempotency-key", key)],
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["proposal_id"], proposal);
}

fn allow_reviewer(repo: &Repository) {
    let mut manifest: Manifest =
        serde_json::from_slice(&std::fs::read(repo.layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        repo.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requirement_get_pairs_content_with_the_etag_from_one_publication_snapshot() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let (status, initial, old_etag) =
        call(&host, "GET", "/requirements/req_shared", None, &[]).await;
    assert_eq!(status, 200, "{initial}");
    let old_etag = old_etag.unwrap();

    let (record_tx, record_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let writer_host = host.clone();
    let writer_etag = old_etag.clone();
    let writer = std::thread::spawn(move || {
        record_rx.recv().unwrap();
        started_tx.send(()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(call(
            &writer_host,
            "PATCH",
            "/requirements/req_shared",
            Some(json!({"data":{"actor":"writer","statement":"The graph changed."}})),
            &[
                ("idempotency-key", "concurrent_save"),
                ("if-match", &writer_etag),
            ],
        ));
        let _ = done_tx.send(());
        result
    });
    let layout = repo.layout.clone();
    fixture_probe::arm("requirement_resource_record_read", move || {
        let snapshot_lock_is_held = fixture_probe::publication_lock_is_held(&layout);
        record_tx.send(()).unwrap();
        started_rx.recv().unwrap();
        if !snapshot_lock_is_held {
            done_rx.recv().unwrap();
        }
    });

    let (read_status, raced, raced_etag) =
        call(&host, "GET", "/requirements/req_shared", None, &[]).await;
    fixture_probe::disarm("requirement_resource_record_read");
    let (write_status, written, _) = writer.join().unwrap();
    assert_eq!(read_status, 200, "{raced}");
    assert_eq!(write_status, 200, "{written}");
    let raced_pair = (
        raced["data"]["statement"].as_str().unwrap(),
        raced_etag.as_deref().unwrap(),
    );
    let new_etag = format!("\"{}\"", written["data"]["edit"]["etag"].as_str().unwrap());
    assert!(
        raced_pair == ("The graph is readable.", old_etag.as_str())
            || raced_pair == ("The graph changed.", new_etag.as_str()),
        "content and ETag came from different snapshots: {raced} {raced_etag:?}"
    );
}

#[tokio::test]
async fn relationship_patches_accept_deltas_and_final_sets_atomically() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    create(&host, "/sources", None, source("source_old", "Old source")).await;
    create(
        &host,
        "/requirements",
        Some("create_req_second"),
        json!({"actor":"agent","id":"req_second","statement":"The second record exists.",
            "status":"active","depends_on":[],"supersedes":[]}),
    )
    .await;
    create(
        &host,
        "/resolutions",
        None,
        resolution("resolution_old", "req_shared"),
    )
    .await;

    for (path, delta, field, added) in [
        (
            "/sources/source_shared",
            json!({"supersedes":{"add":["source_old"]}}),
            "supersedes",
            "source_old",
        ),
        (
            "/rules/rule_shared",
            json!({"requirement_ids":{"add":["req_second"]}}),
            "requirement_ids",
            "req_second",
        ),
        (
            "/resolutions/resolution_shared",
            json!({"supersedes":{"add":["resolution_old"]},"requirement_ids":{"add":["req_second"]}}),
            "supersedes",
            "resolution_old",
        ),
    ] {
        let (status, value) = patch(&host, path, delta).await;
        assert_eq!(status, 200, "{path}: {value}");
        assert!(value["data"][field]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == added));
    }

    let (status, source_removed) = patch(
        &host,
        "/sources/source_shared",
        json!({"supersedes":{"remove":["source_old"]}}),
    )
    .await;
    assert_eq!(status, 200, "{source_removed}");
    assert!(source_removed["data"]["supersedes"].is_null());
    let (status, source_replaced) = patch(
        &host,
        "/sources/source_shared",
        json!({"supersedes":["source_old"]}),
    )
    .await;
    assert_eq!(status, 200, "{source_replaced}");
    assert_eq!(source_replaced["data"]["supersedes"], json!(["source_old"]));

    let (status, resolution_replaced) = patch(
        &host,
        "/resolutions/resolution_shared",
        json!({"requirement_ids":["req_second"],"supersedes":[]}),
    )
    .await;
    assert_eq!(status, 200, "{resolution_replaced}");
    assert_eq!(
        resolution_replaced["data"]["requirement_ids"],
        json!(["req_second"])
    );
    assert!(resolution_replaced["data"]["supersedes"].is_null());

    let (status, replaced) = patch(
        &host,
        "/rules/rule_shared",
        json!({"requirement_ids":["req_second"],"resolution_ids":["resolution_shared"]}),
    )
    .await;
    assert_eq!(status, 200, "{replaced}");
    assert_eq!(replaced["data"]["requirement_ids"], json!(["req_second"]));

    let (status, refused) = patch(
        &host,
        "/rules/rule_shared",
        json!({"statement":"Must not publish.","requirement_ids":[]}),
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    let (_, after, _) = call(&host, "GET", "/rules/rule_shared", None, &[]).await;
    assert_ne!(after["data"]["statement"], "Must not publish.");
    assert_eq!(after["data"]["requirement_ids"], json!(["req_second"]));
}

#[tokio::test]
async fn resolution_context_is_record_data_but_object_context_is_identity_injection() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let mut declared = resolution("resolution_context", "req_shared");
    declared["context"] = json!("Codebase scan");
    let created = create(&host, "/resolutions", None, declared).await;
    assert_eq!(created["data"]["context"], "Codebase scan");

    let mut injected = resolution("resolution_injected", "req_shared");
    injected["context"] = json!({"repository":"other","scope":"default"});
    let (status, failure, _) = call(
        &host,
        "POST",
        "/resolutions",
        Some(json!({"data":injected})),
        &[],
    )
    .await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input");
    assert_eq!(failure["error"]["field"], "context");
}

#[tokio::test]
async fn review_decide_dispatches_and_checks_the_addressed_requirement() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo);
    submit(&host, "proposal_decide", "submit_decide").await;
    create(
        &host,
        "/requirements",
        Some("create_req_other"),
        json!({"actor":"agent","id":"req_other","statement":"The other record exists.",
            "status":"active","depends_on":[],"supersedes":[]}),
    )
    .await;
    let decision = json!({"data":{
        "actor":{"identity_type":"human","id":"reviewer"},
        "disposition_id":"disposition_decide","decision":"rejected",
        "rationale":"The revision needs work.","canonical_artifact":null,
        "feedback":null,"declared_by":null
    }});
    let (wrong_status, wrong, _) = call(
        &host,
        "POST",
        "/requirements/req_other/submissions/proposal_decide/decide",
        Some(decision.clone()),
        &[("idempotency-key", "wrong_parent_decide")],
    )
    .await;
    assert_eq!(wrong_status, 400, "{wrong}");
    assert_eq!(wrong["error"]["kind"], "invalid_update");
    let (status, value, _) = call(
        &host,
        "POST",
        "/requirements/req_shared/submissions/proposal_decide/decide",
        Some(decision),
        &[("idempotency-key", "decide_submission")],
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["fact"], "decided");
    assert_eq!(value["data"]["requirement_id"], "req_shared");
}

#[tokio::test]
async fn review_withdraw_dispatches_for_a_real_submission() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    submit(&host, "proposal_withdraw", "submit_withdraw").await;
    let (status, value, _) = call(
        &host,
        "POST",
        "/requirements/req_shared/submissions/proposal_withdraw/withdraw",
        Some(json!({"data":{"actor":"agent","declared_by":null,"reason":"Revise it."}})),
        &[("idempotency-key", "withdraw_submission")],
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["fact"], "withdrawn");
    assert_eq!(value["data"]["requirement_id"], "req_shared");
}

#[tokio::test]
async fn list_cursor_refuses_a_revision_changed_by_an_insert_before_the_window() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    create(&host, "/sources", None, source("source_z", "Last source")).await;
    let (status, first, _) = call(&host, "GET", "/sources?limit=1", None, &[]).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["data"]["items"][0]["id"], "source_shared");
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();
    assert!(cursor.contains('.'), "cursor is not signed: {cursor}");
    assert!(!cursor.starts_with("list:"), "plain offset cursor escaped");
    create(&host, "/sources", None, source("source_a", "First source")).await;

    let (status, failure, _) = call(
        &host,
        "GET",
        &format!("/sources?limit=1&cursor={cursor}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, 409, "{failure}");
    assert_eq!(failure["error"]["kind"], "cursor_revision_changed");

    let (restart_status, restarted, _) = call(&host, "GET", "/sources?limit=1", None, &[]).await;
    assert_eq!(restart_status, 200, "{restarted}");
    assert_eq!(restarted["data"]["items"][0]["id"], "source_a");
}
