#![cfg(feature = "test-fixture")]

mod support {
    pub mod api_fixture;
    pub mod records;
}

use provenance_porcelain::api::{ApiArguments, ApiMethod, ApiOutcome, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::HostApiPort;
use provenance_transport::StatementHost;
use serde_json::json;
use support::api_fixture::{access, error_kind, host, ApiSession, Repository};

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
    let no_idempotency = session        .call(json!({
            "path": "requirements",
            "method": "post",
            "body": {"id": "req_api", "statement": "One statement.", "actor": "api",
                     "status": "active", "depends_on": [], "supersedes": []}
        }))
        .await;
    assert_eq!(no_idempotency.is_error, Some(true));
    assert_eq!(error_kind(&no_idempotency), "invalid_input");

    // update-requirement requires If-Match and an Idempotency-Key.
    let no_version = session
        .call(json!({
            "path": "requirements/req_shared",
            "method": "patch",
            "headers": {"Idempotency-Key": "api-patch-one"},
            "body": {"description": "One description."}
        }))
        .await;
    assert_eq!(no_version.is_error, Some(true));
    assert_eq!(error_kind(&no_version), "invalid_input");

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
    let through_mcp = session.call(json!({"path": "requirements/req_shared"})).await;

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
async fn api_query_selector_invocations_follow_the_variant_contracts() {
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

    let wrong_selector = session
        .call(json!({"path": "requirements/req_shared", "query": {"query": "stale"}}))
        .await;
    assert_eq!(wrong_selector.is_error, Some(true), "{wrong_selector:?}");
    assert_eq!(error_kind(&wrong_selector), "invalid_input");

    session.shutdown().await;
}
