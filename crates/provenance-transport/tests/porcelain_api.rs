#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_porcelain::api::{ApiArguments, ApiMethod, ApiOutcome, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_store::operations::catalog;
use provenance_transport::fixture::{FixtureAccess, Target};
use provenance_transport::porcelain::HostApiPort;
use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

fn access(repository: &Repository) -> FixtureAccess {
    FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
}

fn host(repository: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repository))
}

async fn call_api_tool(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    arguments: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new("api")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

fn error_kind(result: &rmcp::model::CallToolResult) -> String {
    result.structured_content.as_ref().unwrap()["error"]["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn api_read_returns_the_named_catalog_tool_result_on_the_same_path() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let host = StatementHost::with_fixture_access(access(&repository).allow_writes());
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let named = client
        .call_tool(
            CallToolRequestParams::new("get-source")
                .with_arguments(json!({"id": "source_shared"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_ne!(named.is_error, Some(true), "{named:?}");
    let shared = call_api_tool(&client, json!({"path": "sources/source_shared"})).await;

    assert_ne!(shared.is_error, Some(true), "{shared:?}");
    assert_eq!(
        shared.structured_content, named.structured_content,
        "one public path gives one result"
    );
    assert_eq!(
        shared.structured_content.as_ref().unwrap()["data"]["id"],
        "source_shared"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn api_selects_methods_bodies_and_headers_for_one_public_mutation() {
    let repository = Repository::new("The shared graph is readable.");
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let host = StatementHost::with_fixture_access(access(&repository).allow_writes());
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let created = call_api_tool(
        &client,
        json!({
            "path": "sources",
            "method": "post",
            "headers": {"Idempotency-Key": "api-create-one"},
            "body": {"id": "source_api", "name": "Created by api", "source_type": "document", "supersedes": []}
        }),
    )
    .await;
    assert_ne!(created.is_error, Some(true), "{created:?}");
    assert_eq!(
        created.structured_content.as_ref().unwrap()["data"]["id"],
        "source_api"
    );

    let reread = call_api_tool(&client, json!({"path": "sources/source_api"})).await;
    assert_eq!(
        reread.structured_content.as_ref().unwrap()["data"]["name"],
        "Created by api"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn api_refuses_an_unknown_path_with_the_canonical_failure() {
    let repository = Repository::new("The shared graph is readable.");
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let bound = host(&repository);
    let server = tokio::spawn(async move { bound.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let refused = call_api_tool(&client, json!({"path": "provenance/unknown"})).await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(error_kind(&refused), "unknown_operation");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn api_refuses_a_supported_path_with_an_unsupported_method() {
    let repository = Repository::new("The shared graph is readable.");
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let bound = host(&repository);
    let server = tokio::spawn(async move { bound.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let refused = call_api_tool(
        &client,
        json!({"path": "requirements/req_shared/submit", "method": "get"}),
    )
    .await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(error_kind(&refused), "method_not_allowed");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn api_mutations_keep_the_operation_specific_preconditions() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let host = StatementHost::with_fixture_access(access(&repository).allow_writes());
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    // create-requirement requires an Idempotency-Key header.
    let no_idempotency = call_api_tool(
        &client,
        json!({
            "path": "requirements",
            "method": "post",
            "body": {"id": "req_api", "statement": "One statement.", "actor": "api",
                     "status": "active", "depends_on": [], "supersedes": []}
        }),
    )
    .await;
    assert_eq!(no_idempotency.is_error, Some(true));
    assert_eq!(error_kind(&no_idempotency), "invalid_input");

    // update-requirement requires If-Match and an Idempotency-Key.
    let no_version = call_api_tool(
        &client,
        json!({
            "path": "requirements/req_shared",
            "method": "patch",
            "headers": {"Idempotency-Key": "api-patch-one"},
            "body": {"description": "One description."}
        }),
    )
    .await;
    assert_eq!(no_version.is_error, Some(true));
    assert_eq!(error_kind(&no_version), "invalid_input");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn api_discovery_describes_the_live_catalog_routes() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let bound = StatementHost::with_fixture_access(access(&repository).allow_writes());
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { bound.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    let tool = tools.iter().find(|tool| tool.name == "api").unwrap();
    for field in ["path", "method", "query", "headers", "body"] {
        assert!(
            tool.input_schema["properties"][field].is_object(),
            "{tool:?}"
        );
    }

    let discovery = call_api_tool(&client, json!({})).await;
    assert_ne!(discovery.is_error, Some(true), "{discovery:?}");
    let routes = discovery.structured_content.as_ref().unwrap()["routes"]
        .as_array()
        .unwrap()
        .clone();

    // The write-granted fixture advertises every registered definition, so
    // the described routes must track that set exactly.
    assert_eq!(
        routes.len(),
        catalog::definitions().len(),
        "one described route per advertised definition"
    );

    let member = routes
        .iter()
        .find(|route| route["path"] == "/sources/{id}" && route["method"] == "get")
        .expect("the source member route is described");
    let definition = catalog::definitions()
        .iter()
        .find(|definition| {
            definition.path == "/sources/{id}" && definition.method == catalog::HttpMethod::Get
        })
        .unwrap();
    assert_eq!(member["description"], definition.description);
    assert_eq!(member["response_schema"], definition.mcp_output_schema());
    let names = member["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(names.contains(&"id"));
    assert!(names.contains(&"limit"));

    let create = routes
        .iter()
        .find(|route| route["path"] == "/sources" && route["method"] == "post")
        .expect("the source creation route is described");
    assert!(create["request_schema"].is_object(), "{create}");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn api_discovery_hides_denied_operations() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let restricted = access(&repository).deny_operation("list-sources");
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let bound = StatementHost::with_fixture_access(restricted);
    let server = tokio::spawn(async move { bound.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let discovery = call_api_tool(&client, json!({})).await;
    let routes = discovery.structured_content.as_ref().unwrap()["routes"]
        .as_array()
        .unwrap();
    assert!(
        routes
            .iter()
            .all(|route| !(route["path"] == "/sources" && route["method"] == "get")),
        "{routes:?}"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
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
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { bound.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let through_mcp = call_api_tool(&client, json!({"path": "requirements/req_shared"})).await;

    let ApiOutcome::Invoked(value) = direct.unwrap() else {
        panic!("a path selects one invocation");
    };
    assert_eq!(through_mcp.structured_content.as_ref().unwrap(), &value);
    assert_eq!(value["data"]["id"], "req_shared");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
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
