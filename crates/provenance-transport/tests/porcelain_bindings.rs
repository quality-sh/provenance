use provenance_macros::verifies;
use serde_json::json;
use std::sync::Arc;
#[cfg(feature = "test-fixture")]
use std::sync::Mutex;

#[cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    pub mod records;
    pub mod resource_http;
}

#[test]
fn statement_host_supplies_the_injected_get_port() {
    fn assert_port<P: provenance_porcelain::get::GetPort>() {}

    assert_port::<provenance_transport::porcelain::HostGetPort>();
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
#[verifies("rule_porcelain_id_needs_no_kind_selector", examples)]
#[verifies("rule_porcelain_get_returns_record", examples)]
async fn host_get_port_returns_a_record_through_the_resource_path() {
    use provenance_porcelain::get::{GetInput, View};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, false);
    let porcelain = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostGetPort::new(host),
    );

    let outcome = porcelain
        .get(GetInput::new("req_shared", View::Record))
        .await
        .unwrap();

    assert_eq!(outcome.record.id().as_str(), "req_shared");
    assert_eq!(outcome.record.node_type(), provenance_core::NodeType::Requirement);
    assert!(outcome.record_metadata.as_ref().unwrap().stamp.is_some());
    let impact = porcelain
        .get(GetInput::new("req_shared", View::Impact))
        .await
        .unwrap();
    assert!(impact.view_metadata().unwrap().stamp.is_some());
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
#[verifies("rule_porcelain_mcp_readable_structured", examples)]
async fn mcp_get_runs_through_the_composed_service() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    let get = tools.iter().find(|tool| tool.name == "get").unwrap();
    let output_schema = get.output_schema.as_ref().expect("get output schema");
    assert_eq!(output_schema["type"], "object");
    assert_eq!(output_schema["additionalProperties"], false);
    assert_eq!(
        output_schema["required"],
        json!(["record", "view", "related", "detail", "bounds"])
    );
    assert_eq!(
        output_schema["properties"]["view"]["enum"],
        json!(["record", "children", "grounding", "impact"])
    );
    assert_eq!(
        output_schema["$defs"]["bounds"]["required"],
        json!([
            "limit",
            "max_depth",
            "has_more",
            "continuation",
            "truncated"
        ])
    );
    let result = client
        .call_tool(
            CallToolRequestParams::new("get")
                .with_arguments(json!({"target":"req_shared"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    let readable = &result.content[0].as_text().unwrap().text;
    assert!(readable.contains("requirement req_shared"), "{readable}");
    assert!(readable.contains("The graph is readable."), "{readable}");
    assert!(readable.contains("view: record"), "{readable}");
    let value = result.structured_content.unwrap();
    assert_eq!(value["record"]["id"], "req_shared");
    assert_eq!(value["record"]["kind"], "requirement");
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn mcp_get_readable_content_includes_related_records_and_bounds() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let result = client
        .call_tool(
            CallToolRequestParams::new("get").with_arguments(
                json!({"target":"req_shared","view":"children","limit":7})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    let readable = &result.content[0].as_text().unwrap().text;
    assert!(readable.contains("view: children"), "{readable}");
    assert!(readable.contains("rule_shared"), "{readable}");
    assert!(readable.contains("bounds:"), "{readable}");
    assert!(readable.contains("limit=7"), "{readable}");
    let structured = result.structured_content.as_ref().unwrap();
    assert_eq!(structured["related"][0]["depth"], 1);
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn mcp_get_rejects_an_unknown_returned_kind() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let result = client
        .call_tool(
            CallToolRequestParams::new("get").with_arguments(
                json!({"target":"req_shared","view":"children","returned_kinds":["invented"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();

    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["error"]["kind"],
        "invalid_options"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
#[test]
fn record_fixture_adds_a_scope_without_reusing_repository_ids() {
    let repository = support::records::Repository::new("The default scope is readable.");

    repository.add_scope("other", "The other scope is readable.");
}

#[test]
#[verifies("rule_porcelain_mcp_readable_structured", examples)]
fn mcp_readable_get_warns_when_the_record_or_view_is_stale() {
    let record = serde_json::from_value(json!({
        "node_type": "requirement", "schema_version": 2, "scope_id": "default",
        "id": "req_stale", "statement": "Keep the graph typed.", "status": "active"
    }))
    .unwrap();
    let outcome = provenance_porcelain::get::GetOutcome {
        record,
        result: provenance_porcelain::get::ViewResult::Children(
            provenance_porcelain::get::Traversal {
                records: Vec::new(),
                bounds: provenance_porcelain::get::Bounds {
                    limit: 50,
                    max_depth: Some(1),
                    has_more: false,
                    continuation: None,
                    truncated: false,
                },
                response_metadata: Some(provenance_core::protocol::ResponseMeta {
                    freshness_error: Some("view catch-up failed".into()),
                    ..Default::default()
                }),
            },
        ),
        record_metadata: Some(provenance_core::protocol::ResponseMeta {
            freshness_error: Some("record catch-up failed".into()),
            ..Default::default()
        }),
    };

    let readable = provenance_transport::porcelain::render_get_readable(&outcome);

    assert!(readable.contains("warning: record freshness: record catch-up failed"));
    assert!(readable.contains("warning: view freshness: view catch-up failed"));
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn mcp_get_keeps_permitted_kinds_without_probing_forbidden_kinds() {
    use provenance_transport::fixture::{FixtureAccess, Target};
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let mut access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap();
    for operation in [
        "get-source",
        "get-resolution",
        "get-rule",
        "get-topic",
        "get-question",
        "get-domain",
        "get-boundary",
    ] {
        access = access.deny_operation(operation);
    }
    let host = provenance_transport::StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    assert!(client
        .list_all_tools()
        .await
        .unwrap()
        .iter()
        .any(|tool| tool.name == "get"));
    let allowed = client
        .call_tool(
            CallToolRequestParams::new("get")
                .with_arguments(json!({"target":"req_shared"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_ne!(allowed.is_error, Some(true));
    let children = client
        .call_tool(
            CallToolRequestParams::new("get").with_arguments(
                json!({"target":"req_shared","view":"children"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(children.structured_content.as_ref().unwrap()["related"]
        .as_array()
        .unwrap()
        .iter()
        .all(|record| record["kind"] == "requirement"));
    let forbidden = client
        .call_tool(
            CallToolRequestParams::new("get").with_arguments(
                json!({"target":"source_shared"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.is_error, Some(true));
    assert_eq!(
        forbidden.structured_content.unwrap()["error"]["kind"],
        "not_found"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

struct CheckFixturePort;

impl provenance_porcelain::check::CheckPort for CheckFixturePort {
    fn run<'a>(
        &'a self,
        category: provenance_porcelain::check::Category,
        _: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        Box::pin(async move {
            let findings = match category {
                provenance_porcelain::check::Category::Statements => {
                    vec![provenance_porcelain::check::Finding::new(
                        "statement finding",
                    )]
                }
                provenance_porcelain::check::Category::Graph
                | provenance_porcelain::check::Category::Bindings => Vec::new(),
            };
            Ok(category_run(category, findings))
        })
    }
}

const fn category_run(
    category: provenance_porcelain::check::Category,
    findings: Vec<provenance_porcelain::check::Finding>,
) -> provenance_porcelain::check::CategoryRun {
    use provenance_porcelain::check::{
        BindingContext, BindingPolicy, Category, CategoryRun, Refusal,
    };
    match category {
        Category::Graph => CategoryRun::Graph {
            findings,
            refusal: Refusal::None,
        },
        Category::Statements => CategoryRun::Statements {
            findings,
            context: None,
            refusal: Refusal::None,
        },
        Category::Bindings => CategoryRun::Bindings {
            findings,
            context: BindingContext {
                policy: BindingPolicy::Warning,
            },
            refusal: Refusal::None,
        },
    }
}

#[tokio::test]
#[verifies("rule_porcelain_check_categories", examples)]
#[verifies("rule_porcelain_mcp_readable_structured", examples)]
async fn mcp_check_uses_its_separately_injected_port() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let host =
        provenance_transport::StatementHost::default().with_check_port(Arc::new(CheckFixturePort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    let check = tools.iter().find(|tool| tool.name == "check").unwrap();
    let output_schema = check.output_schema.as_ref().expect("check output schema");
    assert_eq!(output_schema["type"], "object");
    assert_eq!(output_schema["additionalProperties"], false);
    assert_eq!(output_schema["required"], json!(["categories"]));
    assert_eq!(
        output_schema["properties"]["categories"]["items"]["properties"]["status"]["enum"],
        json!(["passed", "findings", "unavailable"])
    );
    assert_eq!(
        output_schema["properties"]["categories"]["items"]["properties"]["findings"]["items"]
            ["required"],
        json!(["message"])
    );
    let result = client
        .call_tool(
            CallToolRequestParams::new("check").with_arguments(
                json!({"categories":["statements"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.as_ref().unwrap()["categories"][0]["category"],
        "statements"
    );
    assert_eq!(
        result.structured_content.as_ref().unwrap()["categories"][0]["status"],
        "findings"
    );
    assert!(result.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("statements: findings"));
    assert!(result.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("statement finding"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn mcp_regular_graph_work_executes_a_record_command() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let result = client
        .call_tool(CallToolRequestParams::new("list-requirements"))
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(
        result.structured_content.unwrap()["data"]["items"][0]["id"],
        "req_shared"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
struct ScopeRecordingPort(Arc<Mutex<Option<String>>>);

#[cfg(feature = "test-fixture")]
impl provenance_porcelain::check::CheckPort for ScopeRecordingPort {
    fn run<'a>(
        &'a self,
        category: provenance_porcelain::check::Category,
        scope: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        *self.0.lock().unwrap() = scope.map(str::to_owned);
        Box::pin(async move { Ok(category_run(category, Vec::new())) })
    }
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn mcp_check_binds_the_host_scope_into_the_check_port() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let seen = Arc::new(Mutex::new(None));
    let host = support::resource_http::host(&repository, false)
        .with_check_port(Arc::new(ScopeRecordingPort(seen.clone())));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    client
        .call_tool(CallToolRequestParams::new("check"))
        .await
        .unwrap();

    assert_eq!(seen.lock().unwrap().as_deref(), Some("default"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
