#![cfg(feature = "test-fixture")]

mod support {
    pub mod api_fixture;
    pub mod records;
}

use provenance_macros::verifies;
use provenance_porcelain::api::{ApiMethod, ApiOutcome, ApiParameter, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::HostApiPort;
use support::api_fixture::{host, Repository};

fn names_of(parameters: &[ApiParameter]) -> Vec<&str> {
    parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect()
}

#[tokio::test]
#[verifies("rule_porcelain_api_catalog_discovery", examples)]
async fn api_discovery_preserves_query_contracts() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let service = Porcelain::new(HostApiPort::new(host(&repository)));

    let ApiOutcome::Catalog(catalog) = service
        .execute_api(ApiRequest::Discover {
            filter: None,
            limit: None,
        })
        .await
        .unwrap()
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
    assert!(symbol_names.contains(&"cursor"));
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
