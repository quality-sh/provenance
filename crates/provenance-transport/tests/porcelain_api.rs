#![cfg(feature = "test-fixture")]

mod support {
    pub mod api_fixture;
    pub mod records;
}

use provenance_macros::verifies;
use provenance_porcelain::api::{ApiArguments, ApiMethod, ApiOutcome, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::HostApiPort;
use provenance_transport::StatementHost;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use support::api_fixture::{access, error_kind, host, ApiSession, Repository};

/// The canonical refusal of one refused tool result.
fn refusal(result: &CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(true), "{result:?}");
    result.structured_content.as_ref().unwrap()["error"].clone()
}

#[tokio::test]
async fn api_read_returns_the_named_catalog_tool_result_on_the_same_path() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    ))
    .await;

    let named = session
        .call_named("get-source", json!({"id": "source_shared"}))
        .await;
    assert_ne!(named.is_error, Some(true), "{named:?}");
    let shared = session.call(json!({"path": "sources/source_shared"})).await;

    assert_ne!(shared.is_error, Some(true), "{shared:?}");
    assert_eq!(
        shared.structured_content, named.structured_content,
        "one public path gives one result"
    );
    assert_eq!(
        shared.structured_content.as_ref().unwrap()["data"]["id"],
        "source_shared"
    );

    session.shutdown().await;
}

#[tokio::test]
#[verifies("rule_porcelain_mcp_api_arguments", examples)]
async fn api_selects_methods_bodies_and_headers_for_one_public_mutation() {
    let repository = Repository::new("The shared graph is readable.");
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    ))
    .await;

    let created = session
        .call(json!({
            "path": "sources",
            "method": "post",
            "headers": {"Idempotency-Key": "api-create-one"},
            "body": {"id": "source_api", "name": "Created by api", "source_type": "document", "supersedes": []}
        }))
        .await;
    assert_ne!(created.is_error, Some(true), "{created:?}");
    assert_eq!(
        created.structured_content.as_ref().unwrap()["data"]["id"],
        "source_api"
    );

    let reread = session.call(json!({"path": "sources/source_api"})).await;
    assert_eq!(
        reread.structured_content.as_ref().unwrap()["data"]["name"],
        "Created by api"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn api_refuses_an_unknown_path_with_the_canonical_failure() {
    let repository = Repository::new("The shared graph is readable.");
    let session = ApiSession::start(host(&repository)).await;

    let refused = session.call(json!({"path": "provenance/unknown"})).await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(error_kind(&refused), "unknown_operation");

    session.shutdown().await;
}

#[tokio::test]
async fn api_method_without_a_path_refuses_like_the_cli() {
    let repository = Repository::new("The shared graph is readable.");
    let session = ApiSession::start(host(&repository)).await;

    let refused = session.call(json!({"method": "post"})).await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(error_kind(&refused), "invalid_input");

    session.shutdown().await;
}

#[tokio::test]
async fn api_refuses_a_supported_path_with_an_unsupported_method() {
    let repository = Repository::new("The shared graph is readable.");
    let session = ApiSession::start(host(&repository)).await;

    // The refusal is per-call dispatch, not a missing tool: the api tool is
    // listed, and the path exists under POST only.
    let listed = session.tools().await.iter().any(|tool| tool.name == "api");
    assert!(listed, "the api tool is listed");

    let refused = session
        .call(json!({"path": "requirements/req_shared/submit", "method": "get"}))
        .await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(error_kind(&refused), "method_not_allowed");

    session.shutdown().await;
}

#[tokio::test]
async fn api_mutations_keep_the_operation_specific_preconditions() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    ))
    .await;

    // create-requirement requires an Idempotency-Key header.
    let no_idempotency = session
        .call(json!({
            "path": "requirements",
            "method": "post",
            "body": {"id": "req_api", "statement": "One statement.", "actor": "api",
                     "status": "active", "depends_on": [], "supersedes": []}
        }))
        .await;
    let refused = refusal(&no_idempotency);
    assert_eq!(refused["kind"], "invalid_input");
    assert_eq!(refused["field"], "Idempotency-Key");

    // update-requirement requires If-Match and an Idempotency-Key.
    let no_version = session
        .call(json!({
            "path": "requirements/req_shared",
            "method": "patch",
            "headers": {"Idempotency-Key": "api-patch-one"},
            "body": {"description": "One description."}
        }))
        .await;
    let refused = refusal(&no_version);
    assert_eq!(refused["kind"], "invalid_input");
    assert_eq!(refused["field"], "If-Match");

    let read = session
        .call(json!({"path": "requirements/req_shared"}))
        .await;
    let etag = read.structured_content.as_ref().unwrap()["data"]["edit"]["etag"]
        .as_str()
        .expect("the requirement read carries its etag")
        .to_owned();
    let edit = |key: &str, description: &str| {
        json!({
            "path": "requirements/req_shared",
            "method": "patch",
            "headers": {"Idempotency-Key": key, "If-Match": etag},
            "body": {"actor": "api", "description": description}
        })
    };

    let current = session
        .call(edit("api-patch-current", "Edited through api."))
        .await;
    assert_ne!(current.is_error, Some(true), "{current:?}");
    assert_eq!(
        current.structured_content.as_ref().unwrap()["data"]["description"],
        "Edited through api."
    );

    let stale = session.call(edit("api-patch-stale", "A stale edit.")).await;
    let named = session
        .call_named(
            "update-requirement",
            json!({
                "id": "req_shared",
                "idempotency_key": "named-patch-stale",
                "if_match": etag,
                "data": {"actor": "api", "description": "A stale edit."}
            }),
        )
        .await;
    assert_eq!(
        refusal(&stale),
        refusal(&named),
        "a stale If-Match refuses as the named tool refuses"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn api_refuses_header_names_that_differ_only_in_case() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    ))
    .await;

    let refused = session
        .call(json!({
            "path": "requirements/req_shared",
            "method": "patch",
            "headers": {"Idempotency-Key": "api-patch-case", "If-Match": "\"1\"", "if-match": "\"2\""},
            "body": {"actor": "api", "description": "One description."}
        }))
        .await;
    let refused = refusal(&refused);
    assert_eq!(refused["kind"], "invalid_input");
    assert_eq!(refused["field"], "headers");

    session.shutdown().await;
}

#[tokio::test]
async fn api_arguments_follow_the_http_router() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    ))
    .await;

    let upper = session
        .call(json!({"path": "sources/source_shared", "method": "GET"}))
        .await;
    assert_ne!(upper.is_error, Some(true), "{upper:?}");

    for path in ["sources/", "//sources", "requirements/req_shared/submit/"] {
        let refused = session.call(json!({"path": path, "method": "post"})).await;
        assert_eq!(refusal(&refused)["kind"], "unknown_operation", "{path}");
    }

    let no_body = session
        .call(json!({
            "path": "sources",
            "method": "post",
            "headers": {"Idempotency-Key": "api-no-body"}
        }))
        .await;
    let refused = refusal(&no_body);
    assert_eq!(refused["kind"], "invalid_input");
    assert_eq!(refused["reason"], "malformed_json");

    let unknown_argument = session
        .call(json!({"path": "sources", "extra": true}))
        .await;
    assert_eq!(refusal(&unknown_argument)["field"], "extra");
    let unknown_method = session
        .call(json!({"path": "sources", "method": "delete"}))
        .await;
    assert_eq!(refusal(&unknown_method)["field"], "method");

    session.shutdown().await;
}

#[tokio::test]
async fn host_api_port_and_the_mcp_api_tool_produce_one_result() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let bound = host(&repository);
    let service = Porcelain::new(HostApiPort::new(bound.clone()));
    let arguments = ApiArguments {
        path: Some("requirements/req_shared".into()),
        ..Default::default()
    };

    let direct = service
        .execute_api(ApiRequest::from(arguments).expect("valid request"))
        .await;
    let session = ApiSession::start(bound).await;
    let through_mcp = session
        .call(json!({"path": "requirements/req_shared"}))
        .await;

    let ApiOutcome::Invoked(value) = direct.unwrap() else {
        panic!("a path selects one invocation");
    };
    assert_eq!(through_mcp.structured_content.as_ref().unwrap(), &value);
    assert_eq!(value["data"]["id"], "req_shared");

    session.shutdown().await;
}

#[tokio::test]
async fn host_api_port_maps_canonical_failures_for_direct_callers() {
    let repository = Repository::new("The shared graph is readable.");
    let service = Porcelain::new(HostApiPort::new(host(&repository)));

    let unknown = service
        .execute_api(
            ApiRequest::from(ApiArguments {
                path: Some("nowhere".into()),
                ..Default::default()
            })
            .expect("valid request"),
        )
        .await
        .unwrap_err();
    assert_eq!(
        unknown.kind,
        provenance_porcelain::api::ApiErrorKind::UnknownPath
    );
    assert_eq!(unknown.failure["error"]["kind"], "unknown_operation");

    let method = service
        .execute_api(
            ApiRequest::from(ApiArguments {
                path: Some("requirements/req_shared/submit".into()),
                method: Some(ApiMethod::Get),
                ..Default::default()
            })
            .expect("valid request"),
        )
        .await
        .unwrap_err();
    assert_eq!(
        method.kind,
        provenance_porcelain::api::ApiErrorKind::MethodNotAllowed
    );
    assert_eq!(method.failure["error"]["kind"], "method_not_allowed");
}

#[tokio::test]
async fn api_query_invocations_follow_their_contracts() {
    let repository = Repository::new("The shared rule is readable.");
    repository.all_kinds();
    let session = ApiSession::start(host(&repository)).await;

    let searched = session
        .call(json!({
            "path": "rules",
            "query": {"query": "search", "text": "readable"}
        }))
        .await;
    assert_ne!(searched.is_error, Some(true), "{searched:?}");
    let items = searched.structured_content.as_ref().unwrap()["data"]["items"]
        .as_array()
        .unwrap();
    assert!(items
        .iter()
        .any(|item| item["id"].as_str() == Some("rule_shared")));

    let neighbors = session
        .call(json!({
            "path": "requirements/req_shared",
            "query": {"query": "neighbors", "limit": "5"}
        }))
        .await;
    assert_ne!(neighbors.is_error, Some(true), "{neighbors:?}");
    assert!(neighbors
        .structured_content
        .as_ref()
        .unwrap()
        .get("data")
        .is_some());

    let unknown_query = session
        .call(json!({"path": "requirements/req_shared", "query": {"query": "stale"}}))
        .await;
    assert_eq!(unknown_query.is_error, Some(true), "{unknown_query:?}");
    assert_eq!(error_kind(&unknown_query), "invalid_input");

    session.shutdown().await;
}
