#![cfg(feature = "test-fixture")]

mod support {
    pub mod api_fixture;
    pub mod records;
}

use jsonschema::JSONSchema;
use provenance_macros::verifies;
use provenance_porcelain::api::{ApiMethod, ApiOutcome, ApiParameter, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_store::operations::catalog;
use provenance_transport::porcelain::HostApiPort;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::api_fixture::{access, error_kind, host, ApiSession, Repository};

fn names_of(parameters: &[ApiParameter]) -> Vec<&str> {
    parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect()
}

#[tokio::test]
#[verifies("rule_porcelain_api_catalog_discovery", examples)]
async fn api_discovery_describes_the_live_catalog_routes() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    ))
    .await;

    let tools = session.tools().await;
    let tool = tools.iter().find(|tool| tool.name == "api").unwrap();
    for field in ["path", "method", "query", "headers", "body"] {
        assert!(
            tool.input_schema["properties"][field].is_object(),
            "{tool:?}"
        );
    }

    let discovery = session.call(json!({})).await;
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
    let forms = member["queries"].as_array().unwrap();
    assert_eq!(forms.len(), 4, "one base and three member queries");
    let base = forms.iter().find(|form| form["query"].is_null()).unwrap();
    let names = base["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, ["id"], "the member base form binds only the path");
    let base_success = base["success_schema"].as_object().unwrap();
    assert_eq!(
        base_success["properties"]["data"]["$ref"],
        json!("#/$defs/Source"),
        "the base form publishes the source-resource envelope"
    );
    assert_eq!(base_success["required"], json!(["data", "meta"]));
    let neighbors = forms
        .iter()
        .find(|form| form["query"] == "neighbors")
        .unwrap();
    assert_ne!(neighbors["success_schema"], base["success_schema"]);

    let create = routes
        .iter()
        .find(|route| route["path"] == "/sources" && route["method"] == "post")
        .expect("the source creation route is described");
    assert!(create["request_schema"].is_object(), "{create}");

    session.shutdown().await;
}

#[tokio::test]
async fn api_discovery_hides_denied_operations() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let restricted = access(&repository).deny_operation("list-sources");
    let session = ApiSession::start(StatementHost::with_fixture_access(restricted)).await;

    let discovery = session.call(json!({})).await;
    let routes = discovery.structured_content.as_ref().unwrap()["routes"]
        .as_array()
        .unwrap();
    assert!(
        routes
            .iter()
            .all(|route| !(route["path"] == "/sources" && route["method"] == "get")),
        "{routes:?}"
    );

    // Hiding the route also refuses the path at invocation.
    let refused = session
        .call(json!({"path": "sources", "query": {"limit": "1"}}))
        .await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(error_kind(&refused), "access_denied");

    session.shutdown().await;
}

#[tokio::test]
#[verifies("rule_porcelain_api_catalog_discovery", examples)]
async fn api_discovery_preserves_query_contracts() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let service = Porcelain::new(HostApiPort::new(host(&repository)));

    let ApiOutcome::Catalog(catalog) = service.execute_api(ApiRequest::Discover).await.unwrap()
    else {
        panic!("discovery returns the catalog");
    };

    let rules = catalog
        .routes
        .iter()
        .find(|route| route.path == "/rules" && route.method == ApiMethod::Get)
        .expect("the rules list route is described");
    let queries = rules
        .queries
        .iter()
        .map(|form| form.query.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        queries,
        vec![None, Some("search"), Some("stale"), Some("resolve-symbol")]
    );

    let base = rules
        .queries
        .iter()
        .find(|form| form.query.is_none())
        .unwrap();
    let base_names = names_of(&base.parameters);
    assert!(base_names.contains(&"limit"));
    assert!(base_names.contains(&"cursor"));
    for flattened in ["base", "head", "symbol", "file", "line", "text"] {
        assert!(
            !base_names.contains(&flattened),
            "the base form must not advertise {flattened}"
        );
    }

    let resolve_symbol = rules
        .queries
        .iter()
        .find(|form| form.query.as_deref() == Some("resolve-symbol"))
        .unwrap();
    let symbol_names = names_of(&resolve_symbol.parameters);
    assert!(symbol_names.contains(&"file"));
    assert!(resolve_symbol
        .parameters
        .iter()
        .any(|parameter| parameter.name == "file" && parameter.required));
    assert!(symbol_names.contains(&"symbol"));
    assert!(
        !symbol_names.contains(&"base"),
        "resolve-symbol must not inherit the stale query parameters"
    );
    assert!(!resolve_symbol.success_schema.is_null());
    assert!(!resolve_symbol.failure_schema.is_null());

    let stale = rules
        .queries
        .iter()
        .find(|form| form.query.as_deref() == Some("stale"))
        .unwrap();
    let stale_names = names_of(&stale.parameters);
    assert!(stale_names.contains(&"base"));
    assert!(stale_names.contains(&"head"));
    assert!(!stale.success_schema.is_null());
    assert!(!stale.failure_schema.is_null());
    assert_ne!(
        stale.success_schema, base.success_schema,
        "each form keeps its own response schema"
    );

    let member = catalog
        .routes
        .iter()
        .find(|route| route.path == "/requirements/{id}" && route.method == ApiMethod::Get)
        .expect("the requirement member route is described");
    let member_queries = member
        .queries
        .iter()
        .map(|form| form.query.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        member_queries,
        vec![None, Some("trace"), Some("neighbors"), Some("impact")]
    );
    let member_base = member
        .queries
        .iter()
        .find(|form| form.query.is_none())
        .unwrap();
    assert!(names_of(&member_base.parameters).contains(&"id"));
}

#[tokio::test]
async fn mcp_api_outputs_validate_against_the_published_schema() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = ApiSession::start(host(&repository)).await;

    let tools = session.tools().await;
    let tool = tools
        .iter()
        .find(|tool| tool.name == "api")
        .expect("the api tool is listed");
    let published_schema = Value::Object(
        tool.output_schema
            .as_ref()
            .expect("the api tool publishes an output schema")
            .as_ref()
            .clone(),
    );
    // jsonschema resolves #/$defs/ references lazily, so a dangling reference
    // compiles and only fails validation of an output that reaches it; the
    // discovery output below carries routes, which reach the route defs.
    let published =
        JSONSchema::compile(&published_schema).expect("the published api output schema compiles");

    let discovery = session.call(json!({})).await;
    let structured = discovery.structured_content.as_ref().unwrap();
    assert!(
        published.is_valid(structured),
        "discovery output violates the published schema: {discovery:?}"
    );

    let invoked = session.call(json!({"path": "sources/source_shared"})).await;
    let structured = invoked.structured_content.as_ref().unwrap();
    assert!(
        published.is_valid(structured),
        "invocation output violates the published schema: {invoked:?}"
    );

    // The described base form schema must also describe the live result
    // that the named catalog tool returns on the same public path.
    let named = session
        .call_named("get-source", json!({"id": "source_shared"}))
        .await;
    let routes = discovery.structured_content.as_ref().unwrap()["routes"]
        .as_array()
        .unwrap();
    let member = routes
        .iter()
        .find(|route| route["path"] == "/sources/{id}" && route["method"] == "get")
        .expect("the source member route is described");
    let base_schema = member["queries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["query"].is_null())
        .expect("the base form is described")["success_schema"]
        .clone();
    let base_contract =
        JSONSchema::compile(&base_schema).expect("the described base form schema compiles");
    assert!(
        base_contract.is_valid(named.structured_content.as_ref().unwrap()),
        "named tool output violates the described route schema: {named:?}"
    );

    session.shutdown().await;
}
