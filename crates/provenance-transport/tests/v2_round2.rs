#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use axum::{body::Body, http::Request};
use provenance_core::{MessageRole, NodeType, ScopeId, StableId, ThreadParent};
use provenance_store::{
    review::{DiscussionAction, WriteDiscussion},
    state_store::{PostMessageInput, StateStore},
};
use provenance_transport::StatementHost;
use serde_json::{json, Value};
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

async fn call(host: &StatementHost, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
    let (body, has_body) = body.map_or_else(
        || (Body::empty(), false),
        |value| (Body::from(value.to_string()), true),
    );
    if has_body {
        request = request.header("content-type", "application/json");
    }
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

fn sid(value: impl AsRef<str>) -> StableId {
    StableId::new(value.as_ref()).unwrap()
}

fn parent() -> ThreadParent {
    ThreadParent {
        node_type: NodeType::Source,
        node_id: sid("source_shared"),
    }
}

#[tokio::test]
async fn discussion_members_are_direct_and_both_message_lists_paginate() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let scope = ScopeId::new("default").unwrap();
    let store = StateStore::new(repo.layout.clone());
    let root = store
        .write_discussion(WriteDiscussion {
            scope_id: scope.clone(),
            parent: parent(),
            request_id: sid("round2_root"),
            actor: "reviewer".into(),
            declared_by: None,
            action: DiscussionAction::Start {
                role: MessageRole::User,
                body: "Root".into(),
            },
        })
        .unwrap();
    let mut last = root.message_id.clone().unwrap();
    for version in 1..=50 {
        let reply = store
            .write_discussion(WriteDiscussion {
                scope_id: scope.clone(),
                parent: parent(),
                request_id: sid(format!("round2_reply_{version}")),
                actor: "reviewer".into(),
                declared_by: None,
                action: DiscussionAction::Reply {
                    discussion_id: root.discussion_id.clone(),
                    expected_version: version,
                    role: MessageRole::Assistant,
                    body: format!("Reply {version}"),
                },
            })
            .unwrap();
        last = reply.message_id.unwrap();
    }

    let host = host(&repo);
    let base = format!(
        "/sources/source_shared/discussions/{}/messages",
        root.discussion_id.as_str()
    );
    let (status, member) = call(&host, "GET", &format!("{base}/{}", last.as_str()), None).await;
    assert_eq!(status, 200, "{member}");
    assert_eq!(member["data"]["id"], last.as_str());

    let (status, first) = call(&host, "GET", &format!("{base}?limit=1"), None).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(first["meta"]["limit"], 1);
    assert_eq!(first["meta"]["has_more"], true);
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();
    let (status, second) = call(
        &host,
        "GET",
        &format!("{base}?limit=1&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{second}");
    assert_eq!(second["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["meta"]["limit"], 1);
    let (status, changed) = call(
        &host,
        "GET",
        &format!("{base}?limit=2&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, 409, "{changed}");
    assert_eq!(changed["error"]["kind"], "cursor_invalid");

    let (status, default_page) = call(&host, "GET", &format!("{base}?limit=50"), None).await;
    assert_eq!(status, 200, "{default_page}");
    assert_eq!(default_page["meta"]["limit"], 50);
    assert_eq!(default_page["meta"]["has_more"], true);
    let default_cursor = default_page["meta"]["next_cursor"].as_str().unwrap();
    let (status, default_second) = call(
        &host,
        "GET",
        &format!("{base}?cursor={default_cursor}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{default_second}");
    assert_eq!(default_second["meta"]["limit"], 50);
    assert_eq!(default_second["meta"]["has_more"], false);
    assert_eq!(default_second["data"]["items"].as_array().unwrap().len(), 1);

    let legacy_first = store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: sid("req_shared"),
            },
            role: MessageRole::User,
            body: "Legacy one".into(),
        })
        .unwrap();
    store
        .post_thread_message(PostMessageInput {
            scope_id: scope,
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: sid("req_shared"),
            },
            role: MessageRole::Assistant,
            body: "Legacy two".into(),
        })
        .unwrap();
    let legacy = format!(
        "/requirements/req_shared/discussion-containers/{}/legacy-messages",
        legacy_first.thread.id.as_str()
    );
    let (status, first) = call(&host, "GET", &format!("{legacy}?limit=1"), None).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(first["meta"]["limit"], 1);
    assert_eq!(first["meta"]["has_more"], true);
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();
    let (status, changed) = call(&host, "GET", &format!("{legacy}?cursor={cursor}"), None).await;
    assert_eq!(status, 409, "{changed}");
    assert_eq!(changed["error"]["kind"], "cursor_invalid");
    let (status, second) = call(
        &host,
        "GET",
        &format!("{legacy}?limit=1&cursor={cursor}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{second}");
    assert_eq!(second["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["meta"]["limit"], 1);
    assert_eq!(second["meta"]["has_more"], false);
    assert!(second["meta"]["next_cursor"].is_null());
}

#[tokio::test]
async fn nullable_patch_fields_distinguish_omission_clear_and_value() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    let set = json!({"data":{"url":"https://example.test/source"}});
    let (status, set) = call(&host, "PATCH", "/sources/source_shared", Some(set)).await;
    assert_eq!(status, 200, "{set}");
    assert_eq!(set["data"]["url"], "https://example.test/source");

    let (status, omitted) = call(
        &host,
        "PATCH",
        "/sources/source_shared",
        Some(json!({"data":{"name":"Renamed source"}})),
    )
    .await;
    assert_eq!(status, 200, "{omitted}");
    assert_eq!(omitted["data"]["url"], "https://example.test/source");

    let (status, cleared) = call(
        &host,
        "PATCH",
        "/sources/source_shared",
        Some(json!({"data":{"url":null}})),
    )
    .await;
    assert_eq!(status, 200, "{cleared}");
    assert!(cleared["data"]["url"].is_null());
}

fn contribution() -> Value {
    json!({
        "id":"contribution_round2",
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "participant_slot":"reviewer", "stance":"support",
        "strongest_finding":"Original finding", "evidence_references":[],
        "material_claims":[], "risks":[], "objections":[], "challenges":[],
        "suggested_artifact_changes":[], "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct evidence"},
        "open_questions":[]
    })
}

fn synthesis() -> Value {
    json!({
        "id":"synthesis_round2",
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "summary":"Original summary", "consensus":[], "contested_claims":[],
        "minority_objections":[], "evidence_gaps":[], "unsupported_speculation":[],
        "open_questions":[], "suggested_artifacts":[], "required_human_decisions":[]
    })
}

#[tokio::test]
async fn contribution_and_synthesis_patches_merge_partial_inputs() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    for (path, data) in [
        ("/contributions", contribution()),
        ("/synthesis-packets", synthesis()),
    ] {
        let (status, value) = call(&host, "POST", path, Some(json!({"data":data}))).await;
        assert_eq!(status, 200, "{value}");
    }

    let (status, contribution) = call(
        &host,
        "PATCH",
        "/contributions/contribution_round2",
        Some(json!({"data":{"strongest_finding":"Revised finding"}})),
    )
    .await;
    assert_eq!(status, 200, "{contribution}");
    assert_eq!(contribution["data"]["strongest_finding"], "Revised finding");
    assert_eq!(contribution["data"]["participant_slot"], "reviewer");

    let (status, synthesis) = call(
        &host,
        "PATCH",
        "/synthesis-packets/synthesis_round2",
        Some(json!({"data":{"summary":"Revised summary"}})),
    )
    .await;
    assert_eq!(status, 200, "{synthesis}");
    assert_eq!(synthesis["data"]["summary"], "Revised summary");
    assert_eq!(synthesis["data"]["target"]["artifact_id"], "req_shared");
}

#[tokio::test]
async fn query_values_follow_declared_types_and_query_routes() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    for text in ["123", "true"] {
        let (status, value) = call(
            &host,
            "GET",
            &format!("/sources?query=search&text={text}"),
            None,
        )
        .await;
        assert_eq!(status, 200, "{value}");
    }
    for path in ["/sources?unknown=value", "/sources?query=stale"] {
        let (status, value) = call(&host, "GET", path, None).await;
        assert_eq!(status, 400, "{value}");
        assert_eq!(value["error"]["kind"], "invalid_input");
    }
    let (status, value) = call(
        &host,
        "GET",
        "/requirements/req_shared?query=trace&limit=1",
        None,
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["nodes"].as_array().unwrap().len(), 1);
}
