#![cfg(feature = "test-fixture")]

mod support {
    pub mod api_fixture;
    pub mod records;
}

use jsonschema::JSONSchema;
use provenance_porcelain::api::{ApiMethod, ApiOutcome, ApiParameter, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_store::operations::catalog;
use provenance_transport::porcelain::HostApiPort;
use serde_json::{json, Value};
use support::api_fixture::{access, host, ApiSession, Repository};

fn names_of(parameters: &[ApiParameter]) -> Vec<&str> {
    parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect()
}

#[tokio::test]
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
    let variants = member["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 4, "one base and three member queries");
    let base = variants
        .iter()
        .find(|variant| variant["selector"].is_null())
        .unwrap();
    let names = base["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, ["id"], "the member base variant binds only the path");
    let base_success = base["success_schema"].as_object().unwrap();
    assert_eq!(
        base_success["properties"]["data"]["$ref"],
        json!("#/$defs/Source"),
        "the base variant publishes the source-resource envelope"
    );
    assert_eq!(base_success["required"], json!(["data", "meta"]));
    let neighbors = variants
        .iter()
        .find(|variant| variant["selector"] == "neighbors")
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

    session.shutdown().await;
}

#[tokio::test]
async fn api_discovery_preserves_query_variant_contracts() {
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
    let selectors = rules
        .variants
        .iter()
        .map(|variant| variant.selector.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        selectors,
        vec![None, Some("search"), Some("stale"), Some("resolve-symbol")]
    );

    let base = rules
        .variants
        .iter()
        .find(|variant| variant.selector.is_none())
        .unwrap();
    let base_names = names_of(&base.parameters);
    assert!(base_names.contains(&"limit"));
    assert!(base_names.contains(&"cursor"));
    for flattened in ["base", "head", "symbol", "file", "line", "text"] {
        assert!(
            !base_names.contains(&flattened),
            "the base variant must not advertise {flattened}"
        );
    }

    let resolve_symbol = rules
        .variants
        .iter()
        .find(|variant| variant.selector.as_deref() == Some("resolve-symbol"))
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
        .variants
        .iter()
        .find(|variant| variant.selector.as_deref() == Some("stale"))
        .unwrap();
    let stale_names = names_of(&stale.parameters);
    assert!(stale_names.contains(&"base"));
    assert!(stale_names.contains(&"head"));
    assert!(!stale.success_schema.is_null());
    assert!(!stale.failure_schema.is_null());
    assert_ne!(
        stale.success_schema, base.success_schema,
        "each variant keeps its own response schema"
    );

    let member = catalog
        .routes
        .iter()
        .find(|route| route.path == "/requirements/{id}" && route.method == ApiMethod::Get)
        .expect("the requirement member route is described");
    let member_selectors = member
        .variants
        .iter()
        .map(|variant| variant.selector.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        member_selectors,
        vec![None, Some("trace"), Some("neighbors"), Some("impact")]
    );
    let member_base = member
        .variants
        .iter()
        .find(|variant| variant.selector.is_none())
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
    // Compilation resolves every #/$defs/ reference in the published schema;
    // a dangling reference fails here before any output is validated.
    let published = JSONSchema::compile(&published_schema)
        .expect("the published api output schema compiles");

    let discovery = session.call(json!({})).await;
    let structured = discovery.structured_content.as_ref().unwrap();
    assert!(
        published.is_valid(structured),
        "discovery output violates the published schema: {discovery:?}"
    );

    let invoked = session
        .call(json!({"path": "sources/source_shared"}))
        .await;
    let structured = invoked.structured_content.as_ref().unwrap();
    assert!(
        published.is_valid(structured),
        "invocation output violates the published schema: {invoked:?}"
    );

    session.shutdown().await;
}
