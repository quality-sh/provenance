use provenance_macros::verifies;
use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde_json::json;
use std::sync::Arc;
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

#[test]
#[verifies("rule_porcelain_mcp_target_argument", examples)]
fn mcp_get_translates_its_structured_target_argument() {
    let arguments: provenance_transport::porcelain::GetArguments =
        serde_json::from_value(json!({"target": "req_alpha"})).unwrap();

    assert_eq!(
        arguments.into_request(),
        RecordRequest::new("req_alpha", Action::Get)
    );
}

#[test]
#[verifies("rule_porcelain_mcp_readable_structured", examples)]
fn mcp_result_contains_readable_and_structured_content() {
    let data = json!({"id": "req_alpha", "kind": "requirement"});
    let outcome = Outcome::new(
        "Requirement req_alpha: Keep the surfaces consistent.",
        data.clone(),
    );

    let result = provenance_transport::porcelain::render(outcome).unwrap();

    assert_eq!(
        result.content[0].as_text().unwrap().text,
        "Requirement req_alpha: Keep the surfaces consistent."
    );
    assert_eq!(result.structured_content, Some(data));
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

    assert_eq!(outcome.record.id, "req_shared");
    assert_eq!(outcome.record.kind, "requirement");
    assert_eq!(outcome.record.value["id"], "req_shared");
    assert!(
        outcome.record_metadata.as_ref().unwrap().is_object(),
        "metadata: {:?}",
        outcome.record_metadata
    );
    let impact = porcelain
        .get(GetInput::new("req_shared", View::Impact))
        .await
        .unwrap();
    assert!(impact
        .view_metadata
        .as_ref()
        .unwrap()
        .get("stamp")
        .is_some());
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
#[verifies("rule_porcelain_mcp_target_argument", examples)]
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
    assert!(tools.iter().any(|tool| tool.name == "get"));
    let result = client
        .call_tool(
            CallToolRequestParams::new("get")
                .with_arguments(json!({"target":"req_shared"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        result.content[0].as_text().unwrap().text,
        "requirement req_shared"
    );
    let value = result.structured_content.unwrap();
    assert_eq!(value["record"]["id"], "req_shared");
    assert_eq!(value["record"]["kind"], "requirement");
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
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
            Ok(match category {
                provenance_porcelain::check::Category::Statements => {
                    vec![provenance_porcelain::check::Finding::new(
                        "statement finding",
                    )]
                }
                provenance_porcelain::check::Category::Graph
                | provenance_porcelain::check::Category::Bindings => Vec::new(),
            })
        })
    }
}

#[tokio::test]
#[verifies("rule_porcelain_check_categories", examples)]
async fn mcp_check_uses_its_separately_injected_port() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let host =
        provenance_transport::StatementHost::default().with_check_port(Arc::new(CheckFixturePort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool.name == "check"));
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
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
struct ScopeRecordingPort(Arc<Mutex<Option<String>>>);

#[cfg(feature = "test-fixture")]
impl provenance_porcelain::check::CheckPort for ScopeRecordingPort {
    fn run<'a>(
        &'a self,
        _: provenance_porcelain::check::Category,
        scope: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        *self.0.lock().unwrap() = scope.map(str::to_owned);
        Box::pin(async { Ok(Vec::new()) })
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
