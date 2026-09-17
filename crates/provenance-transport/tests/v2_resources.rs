#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
}
use axum::{body::Body, http::Request};
use provenance_core::{MessageRole, NodeType, ScopeId, StableId, ThreadParent};
use provenance_store::state_store::{PostMessageInput, StateStore};
use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;
use tower::ServiceExt as _;

fn host(repo: &Repository, writable: bool) -> StatementHost {
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
    .unwrap();
    StatementHost::with_fixture_access(if writable {
        access.allow_writes()
    } else {
        access
    })
}

async fn call(host: &StatementHost, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    call_with_headers(host, method, path, body, &[]).await
}

#[allow(clippy::option_if_let_else)]
async fn call_with_headers(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (u16, Value) {
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
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}

#[tokio::test]
async fn all_addressed_discussion_routes_work_for_questions() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let parent = "/questions/question_shared/discussions";
    let start = json!({"data":{
        "actor":"reviewer", "declared_by":null, "role":"user", "body":"First message."
    }});
    let (status, started) = call_with_headers(
        &host,
        "POST",
        parent,
        Some(start),
        &[("idempotency-key", "discussion_start")],
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let discussion_id = started["data"]["discussion_id"].as_str().unwrap();
    let root_message_id = started["data"]["message_id"].as_str().unwrap();

    let (status, listed) = call(&host, "GET", parent, None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["data"]["items"].as_array().unwrap().len(), 1);

    let (status, missing) = call(
        &host,
        "GET",
        "/questions/question_shared/discussions/discussion_missing",
        None,
    )
    .await;
    assert_eq!(status, 404, "{missing}");
    assert_eq!(missing["error"]["kind"], "resource_not_found");

    let discussion = format!("{parent}/{discussion_id}");
    let (status, read) = call(&host, "GET", &discussion, None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read["data"]["discussion"]["discussion_id"], discussion_id);

    let messages = format!("{discussion}/messages");
    let (status, first_page) = call(&host, "GET", &messages, None).await;
    assert_eq!(status, 200, "{first_page}");
    assert_eq!(first_page["data"]["items"].as_array().unwrap().len(), 1);

    let (status, root_message) =
        call(&host, "GET", &format!("{messages}/{root_message_id}"), None).await;
    assert_eq!(status, 200, "{root_message}");
    assert_eq!(root_message["data"]["id"], root_message_id);

    let reply = json!({"data":{
        "actor":"reviewer", "declared_by":null, "role":"assistant", "body":"Reply."
    }});
    let (status, replied) = call_with_headers(
        &host,
        "POST",
        &messages,
        Some(reply),
        &[
            ("idempotency-key", "discussion_reply"),
            ("if-match", "\"1\""),
        ],
    )
    .await;
    assert_eq!(status, 200, "{replied}");
    assert_eq!(replied["data"]["version"], 2);

    let update = json!({"data":{
        "actor":"reviewer", "declared_by":null, "status":"resolved"
    }});
    let (status, resolved) = call_with_headers(
        &host,
        "PATCH",
        &discussion,
        Some(update),
        &[
            ("idempotency-key", "discussion_resolve"),
            ("if-match", "\"2\""),
        ],
    )
    .await;
    assert_eq!(status, 200, "{resolved}");
    assert_eq!(resolved["data"]["status"], "resolved");
}

#[tokio::test]
async fn a_discussion_member_read_is_addressed_beyond_the_first_list_page() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let parent = "/sources/source_shared/discussions";
    let mut ids = Vec::new();

    for index in 0..51 {
        let (status, started) = call_with_headers(
            &host,
            "POST",
            parent,
            Some(json!({"data":{
                "actor":"reviewer", "declared_by":null,
                "role":"user", "body":format!("Discussion {index}.")
            }})),
            &[("idempotency-key", &format!("discussion_{index}"))],
        )
        .await;
        assert_eq!(status, 200, "{started}");
        ids.push(
            started["data"]["discussion_id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }

    let discussion_id = ids.into_iter().max().unwrap();
    let (status, read) = call(
        &host,
        "GET",
        &format!("{parent}/{discussion_id}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read["data"]["discussion"]["discussion_id"], discussion_id);
}

#[tokio::test]
async fn draft_patch_refuses_missing_resources() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let contribution = json!({"data":{
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "participant_slot":"reviewer", "stance":"support",
        "strongest_finding":"Observed.", "evidence_references":[], "material_claims":[],
        "risks":[], "objections":[], "challenges":[], "suggested_artifact_changes":[],
        "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct evidence."}, "open_questions":[]
    }});
    let synthesis = json!({"data":{
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "summary":"No record exists.", "consensus":[], "contested_claims":[],
        "minority_objections":[], "evidence_gaps":[], "unsupported_speculation":[],
        "open_questions":[], "suggested_artifacts":[], "required_human_decisions":[]
    }});
    for (path, body) in [
        ("/contributions/contribution_missing", contribution),
        ("/synthesis-packets/synthesis_missing", synthesis),
    ] {
        let (status, failure) = call(&host, "PATCH", path, Some(body)).await;
        assert_eq!(status, 404, "{failure}");
        assert_eq!(failure["error"]["kind"], "resource_not_found");
        let (status, read) = call(&host, "GET", path, None).await;
        assert_eq!(status, 404, "{read}");
    }
}

#[tokio::test]
async fn list_controls_paginate_and_member_controls_are_rejected() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let second = json!({"data":{
        "actor":"reviewer", "id":"req_second", "statement":"The second record is readable.",
        "status":"discovery", "depends_on":[], "supersedes":[]
    }});
    let (status, created) = call_with_headers(
        &host,
        "POST",
        "/requirements",
        Some(second),
        &[("idempotency-key", "create_second_requirement")],
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let (status, first) = call(&host, "GET", "/requirements?limit=1", None).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["meta"]["limit"], 1);
    assert_eq!(first["meta"]["has_more"], true);
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();
    let (status, second) = call(
        &host,
        "GET",
        &format!("/requirements?limit=1&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{second}");
    assert_eq!(second["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["meta"]["has_more"], false);

    let (status, failure) = call(&host, "GET", "/requirements/req_shared?limit=1", None).await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input");
}

#[tokio::test]
async fn review_read_refusals_use_honest_statuses_and_kinds() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (status, missing) = call(
        &host,
        "GET",
        "/requirements/req_shared/history/entry_missing",
        None,
    )
    .await;
    assert_eq!(status, 404, "{missing}");
    assert_eq!(missing["error"]["kind"], "resource_not_found");

    let (status, invalid) = call(
        &host,
        "GET",
        "/requirements/req_shared/history/entry_missing/evidence/sideways",
        None,
    )
    .await;
    assert_eq!(status, 400, "{invalid}");
    assert_eq!(invalid["error"]["kind"], "invalid_input");
}

#[tokio::test]
async fn legacy_message_member_requires_the_addressed_parent() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let scope = ScopeId::new("default").unwrap();
    let posted = StateStore::new(repo.layout.clone())
        .post_thread_message(PostMessageInput {
            scope_id: scope,
            parent: ThreadParent {
                node_type: NodeType::Source,
                node_id: StableId::new("source_shared").unwrap(),
            },
            role: MessageRole::User,
            body: "Owned by the source.".into(),
        })
        .unwrap();
    let host = host(&repo, false);
    let path = format!(
        "/questions/question_shared/discussion-containers/{}/legacy-messages/{}",
        posted.thread.id.as_str(),
        posted.message.id.as_str()
    );

    let (status, failure) = call(&host, "GET", &path, None).await;

    assert_eq!(status, 404, "{failure}");
    assert_eq!(failure["error"]["kind"], "resource_not_found");
}

#[tokio::test]
async fn collection_member_query_and_write_use_the_bound_identity() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (status, metadata) = call(&host, "GET", "/metadata", None).await;
    assert_eq!(status, 200, "{metadata}");
    assert_eq!(metadata["data"]["repository"], "selected");
    assert_eq!(metadata["data"]["scope"], "default");

    let (status, listed) = call(&host, "GET", "/sources", None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["data"]["items"][0]["id"], "source_shared");
    assert_eq!(listed["data"]["items"][0]["scope_id"], "default");

    let (status, member) = call(&host, "GET", "/rules/rule_shared", None).await;
    assert_eq!(status, 200, "{member}");
    assert_eq!(member["data"]["id"], "rule_shared");

    let (status, search) = call(&host, "GET", "/rules?query=search&text=shared", None).await;
    assert_eq!(status, 200, "{search}");
    assert!(search["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == "rule_shared"));

    let source = json!({"data":{
        "id":"source_created", "name":"Created source", "source_type":"document",
        "url":null, "reference":null, "commit_pin":null, "effective_date":null,
        "review_date":null, "supersedes":[], "origin_thread":null, "origin_message":null
    }});
    let (status, created) = call(&host, "POST", "/sources", Some(source)).await;
    assert_eq!(status, 200, "{created}");
    assert_eq!(created["data"]["scope_id"], "default");
    assert_eq!(created["data"]["id"], "source_created");
}

#[tokio::test]
async fn authentication_host_and_origin_checks_precede_body_decoding() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    for (credential, authority, origin, status, kind) in [
        ("", "fixture.test", None, 401, "unauthenticated"),
        (
            "Bearer invalid",
            "fixture.test",
            None,
            401,
            "unauthenticated",
        ),
        (
            "Bearer fixture-secret",
            "evil.test",
            None,
            403,
            "access_denied",
        ),
        (
            "Bearer fixture-secret",
            "fixture.test",
            Some("https://evil.test"),
            403,
            "access_denied",
        ),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/statement-checks")
            .header("host", authority)
            .header("authorization", credential);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        let response = host
            .router()
            .oneshot(request.body(Body::from("invalid json")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status);
        let value: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["error"]["kind"], kind);
        assert_eq!(value["meta"], json!({}));
    }
}

#[tokio::test]
async fn mcp_keeps_role_subsets_and_returns_the_http_envelope() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool.name == "list-sources"));
    assert!(!tools.iter().any(|tool| tool.name == "create-source"));
    assert!(tools
        .iter()
        .all(|tool| tool.description.as_deref() != Some("Invoke the shared operation.")));
    let result = client
        .call_tool(CallToolRequestParams::new("list-sources"))
        .await
        .unwrap();
    let value = result.structured_content.unwrap();
    assert!(value["data"]["items"].is_array());
    assert!(value["meta"].is_object());
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn requirement_patch_binds_receipt_and_precondition_headers() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let request = Request::builder()
        .method("GET")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .body(Body::empty())
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let etag = response
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let request = Request::builder()
        .method("PATCH")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .header("idempotency-key", "request_transport_edit")
        .header("if-match", etag)
        .body(Body::from(
            json!({"data":{"actor":"ben","description":"Edited through the resource route."}})
                .to_string(),
        ))
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let value: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status, 200, "{value}");
    assert_eq!(
        value["data"]["description"],
        "Edited through the resource route."
    );
    assert!(value["data"]["edit"]["etag"].is_string());
    assert!(value["data"]["decision"].is_object());
}
