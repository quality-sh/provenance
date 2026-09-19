use provenance_macros::verifies;
use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde_json::json;

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
