use provenance_macros::verifies;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckPort, Finding, GapsContext,
    PortFuture, Refusal,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    #[cfg(feature = "test-fixture")]
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    #[cfg(feature = "test-fixture")]
    #[path = "../../../provenance-transport/tests/support/resource_http.rs"]
    pub mod resource_http;
}

/// Each supplied statement, with the scope the host bound.
type SeenStatements = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// Records each supplied statement with the scope the host bound.
struct StatementRecordingPort(SeenStatements);

impl CheckPort for StatementRecordingPort {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move { Ok(category_run(category, Vec::new())) })
    }

    fn check_statement<'a>(&'a self, statement: &'a str, scope: Option<&'a str>) -> PortFuture<'a> {
        self.0
            .lock()
            .unwrap()
            .push((statement.to_owned(), scope.map(str::to_owned)));
        Box::pin(async move {
            Ok(category_run(
                Category::Statements,
                vec![Finding::new("supplied statement finding")],
            ))
        })
    }
}

const fn category_run(category: Category, findings: Vec<Finding>) -> CategoryRun {
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
        Category::Gaps => CategoryRun::Gaps {
            findings,
            context: GapsContext { scopes: Vec::new() },
        },
        Category::Docs => CategoryRun::Docs {
            findings,
            refusal: Refusal::None,
        },
    }
}

fn check_call(arguments: serde_json::Value) -> CallToolRequestParams {
    let serde_json::Value::Object(arguments) = arguments else {
        panic!("check arguments must be a JSON object");
    };
    CallToolRequestParams::new("check").with_arguments(arguments)
}

#[tokio::test]
#[verifies("rule_porcelain_check_statement_field_shared", examples)]
async fn mcp_check_takes_a_statement_and_refuses_the_cli_only_options() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let host = provenance_transport::StatementHost::default()
        .with_check_port(Arc::new(StatementRecordingPort(seen.clone())));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    let check = tools.iter().find(|tool| tool.name == "check").unwrap();
    assert!(check.input_schema["properties"]["statement"].is_object());
    assert!(check.description.as_deref().unwrap().contains("statement"));
    let alone = client
        .call_tool(check_call(json!({"statement": "The pump stops."})))
        .await
        .unwrap();
    let with_graph = client
        .call_tool(check_call(
            json!({"statement": "The pump stops.", "graph": true}),
        ))
        .await
        .unwrap();

    assert_ne!(alone.is_error, Some(true));
    let categories = &alone.structured_content.as_ref().unwrap()["categories"];
    assert_eq!(categories.as_array().unwrap().len(), 1);
    assert_eq!(categories[0]["category"], "statements");
    assert_eq!(
        categories[0]["findings"][0]["message"],
        "supplied statement finding"
    );
    let categories = &with_graph.structured_content.as_ref().unwrap()["categories"];
    assert_eq!(categories[0]["category"], "graph");
    assert_eq!(categories[1]["category"], "statements");
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            ("The pump stops.".to_owned(), None),
            ("The pump stops.".to_owned(), None),
        ]
    );
    for (arguments, field) in [
        (
            json!({"statement": "The pump stops.", "categories": ["statements"]}),
            "categories",
        ),
        (
            json!({"statement": "The pump stops.", "repo": "."}),
            "repo",
        ),
        (
            json!({"statement": "The pump stops.", "format": "json"}),
            "format",
        ),
    ] {
        let refused = client
            .call_tool(check_call(arguments.clone()))
            .await
            .unwrap();
        assert_eq!(refused.is_error, Some(true), "{arguments}");
        let error = refused.structured_content.unwrap()["error"].clone();
        assert_eq!(
            (&error["kind"], &error["field"], &error["reason"]),
            (
                &json!("invalid_input"),
                &json!(field),
                &json!("unknown_field")
            ),
            "{arguments}"
        );
    }
    assert_eq!(
        seen.lock().unwrap().len(),
        2,
        "a refused call reaches no port"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
#[verifies("rule_porcelain_check_unavailable", examples)]
async fn mcp_check_with_a_statement_is_unavailable_on_a_port_without_that_check() {
    let host = provenance_transport::StatementHost::default()
        .with_check_port(Arc::new(support::check_port::FixtureCheckPort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let result = client
        .call_tool(check_call(json!({"statement": "The pump stops."})))
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    let category = &result.structured_content.as_ref().unwrap()["categories"][0];
    assert_eq!(category["status"], "unavailable");
    assert_eq!(
        category["unavailable_reason"],
        "this host does not check a supplied statement"
    );
    assert!(result.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("statements: unavailable"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
#[verifies("rule_porcelain_check_statement_field_shared", examples)]
async fn mcp_check_binds_the_host_scope_into_the_supplied_statement_check() {
    let repository = support::records::Repository::new("The shared graph is readable.");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let host = support::resource_http::host(&repository, false)
        .with_check_port(Arc::new(StatementRecordingPort(seen.clone())));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    client
        .call_tool(check_call(json!({"statement": "The pump stops."})))
        .await
        .unwrap();

    assert_eq!(
        *seen.lock().unwrap(),
        vec![("The pump stops.".to_owned(), Some("default".to_owned()))]
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
