//! Catalog discovery in the porcelain: the filter, the limit, the default limit, the
//! bounds of the answer, and the readable summary.

#[path = "support/api_routes.rs"]
mod api_routes;

use api_routes::{form, parameter, route};
use provenance_macros::verifies;
use provenance_porcelain::api::{
    render_discovery_readable, ApiCatalog, ApiInput, ApiMethod, ApiOutcome, ApiPort, ApiPortFuture,
    ApiQuery, ApiRequest, CatalogBounds, CatalogSelection, SeeMore, DEFAULT_DISCOVERY_LIMIT,
};
use provenance_porcelain::Porcelain;
use serde_json::{json, Value};

/// A port whose catalog has `count` routes. Each second route reads a source.
struct CatalogPort(usize);

impl ApiPort for CatalogPort {
    fn invoke(&self, _input: ApiInput) -> ApiPortFuture<'_, Value> {
        Box::pin(async { Ok(Value::Null) })
    }

    fn discover(&self) -> ApiCatalog {
        ApiCatalog {
            routes: (0..self.0)
                .map(|index| {
                    let description = if index % 2 == 0 {
                        "List requirements."
                    } else {
                        "Read one source."
                    };
                    route(ApiMethod::Get, &format!("/r{index}"), description, None)
                })
                .collect(),
        }
    }
}

async fn discover(count: usize, filter: Option<&str>, limit: Option<usize>) -> CatalogSelection {
    let request = ApiRequest::Discover {
        filter: filter.map(str::to_owned),
        limit,
    };
    match Porcelain::new(CatalogPort(count))
        .execute_api(request)
        .await
        .unwrap()
    {
        ApiOutcome::Catalog(selection) => selection,
        ApiOutcome::Invoked(value) => panic!("discovery returns the catalog: {value}"),
    }
}

const SEE_MORE: SeeMore = SeeMore {
    filter: "FILTER",
    limit: "LIMIT",
    route: "ROUTE",
};

#[tokio::test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
async fn discovery_reports_the_bounds_of_the_filter_limit_and_default_limit() {
    let bounds = |total, matching, returned| CatalogBounds {
        total,
        matching,
        returned,
        truncated: returned < matching,
    };
    let limited = discover(60, Some("SOURCE"), Some(2)).await;
    assert_eq!(limited.clone().complete(), limited.clone().bounded());
    let answer = limited.complete();
    assert_eq!(answer.bounds, bounds(60, 30, 2));
    assert_eq!(
        answer
            .routes
            .iter()
            .map(|route| route.path.as_str())
            .collect::<Vec<_>>(),
        ["/r1", "/r3"]
    );
    assert_eq!(
        answer.see_more(SEE_MORE).as_deref(),
        Some("Use FILTER or LIMIT to see more.")
    );

    let unlimited = discover(60, None, None).await;
    let complete = unlimited.clone().complete();
    assert_eq!(complete.bounds, bounds(60, 60, 60));
    assert_eq!(complete.see_more(SEE_MORE), None);
    let bounded = unlimited.bounded();
    assert_eq!(bounded.bounds, bounds(60, 60, DEFAULT_DISCOVERY_LIMIT));
    assert_eq!(bounded.routes, complete.routes[..DEFAULT_DISCOVERY_LIMIT]);

    let above_default = discover(60, None, Some(55)).await.bounded();
    assert_eq!(above_default.bounds, bounds(60, 60, 55));
    let under_default = discover(10, None, None).await.bounded();
    assert_eq!(under_default.bounds, bounds(10, 10, 10));
    assert!(render_discovery_readable(&under_default, SEE_MORE)
        .ends_with("inputs: none\nUse ROUTE to see the schemas of one route."));
}

#[test]
fn readable_discovery_lists_routes_with_their_inputs() {
    let mut member = route(
        ApiMethod::Get,
        "/requirements/{id}",
        "Read one requirement.",
        None,
    );
    member.queries[0].parameters = vec![parameter("id", "path", true)];
    member.queries.push(ApiQuery {
        parameters: vec![
            parameter("id", "path", true),
            parameter("query", "query", true),
            parameter("depth", "query", false),
        ],
        ..form(Some("neighbors"))
    });
    let mut create = route(
        ApiMethod::Post,
        "/requirements",
        "Create one requirement.",
        Some(json!({"type": "object"})),
    );
    create.queries[0].parameters = vec![parameter("Idempotency-Key", "header", true)];
    let catalog = ApiCatalog {
        routes: vec![
            route(ApiMethod::Get, "/requirements", "List requirements.", None),
            member,
            create,
        ],
    };

    assert_eq!(
        render_discovery_readable(&catalog.clone().select(None, None).complete(), SEE_MORE),
        concat!(
            "api routes: 3\n",
            "- GET /requirements\n  List requirements.\n  inputs: none\n",
            "- GET /requirements/{id}\n  Read one requirement.\n",
            "  inputs: id (path, required)\n",
            "  inputs with query=neighbors: id (path, required), query (query, required), ",
            "depth (query, optional)\n",
            "- POST /requirements\n  Create one requirement.\n",
            "  inputs: Idempotency-Key (header, required)\n",
            "  The JSON body is the data object, without the HTTP {\"data\": ...} wrapper.\n",
            "Use ROUTE to see the schemas of one route."
        )
    );
    let limited = render_discovery_readable(&catalog.select(None, Some(2)).complete(), SEE_MORE);
    assert!(
        limited.ends_with(concat!(
            "depth (query, optional)\nUse FILTER or LIMIT to see more.\n",
            "Use ROUTE to see the schemas of one route."
        )),
        "{limited}"
    );
}

#[tokio::test]
#[verifies("rule_api_catalog_filter_selects_one_route", examples)]
async fn a_filter_that_is_the_method_and_path_of_one_route_selects_only_that_route() {
    let paths = |selection: CatalogSelection| {
        selection
            .complete()
            .routes
            .into_iter()
            .map(|route| route.path)
            .collect::<Vec<_>>()
    };
    // The filter "get /r1" is also a part of "get /r10" to "get /r19".
    assert_eq!(paths(discover(20, Some("GET /r1"), None).await), ["/r1"]);
    assert_eq!(paths(discover(20, Some("get /R1"), None).await), ["/r1"]);
    // Without a route of that method and path, the filter selects each route that
    // contains it.
    assert_eq!(paths(discover(20, Some("/r1"), None).await).len(), 11);
    assert_eq!(paths(discover(20, Some("post /r1"), None).await).len(), 0);
}

#[test]
#[verifies("rule_api_catalog_schemas_only_for_one_route", examples)]
fn the_json_form_has_schemas_only_when_the_answer_lists_one_route() {
    let mut member = route(ApiMethod::Get, "/sources/{id}", "Read one source.", None);
    member.queries[0].parameters = vec![parameter("id", "path", true)];
    let create = route(
        ApiMethod::Post,
        "/sources",
        "Create one source.",
        Some(json!({"type": "object"})),
    );
    let catalog = ApiCatalog {
        routes: vec![member, create],
    };

    let listing = serde_json::to_value(catalog.clone().select(None, None).complete()).unwrap();
    assert_eq!(
        listing["routes"],
        json!([
            {
                "method": "get",
                "path": "/sources/{id}",
                "description": "Read one source.",
                "queries": [{
                    "query": null,
                    "parameters": [{"name": "id", "location": "path", "required": true}],
                }],
            },
            {
                "method": "post",
                "path": "/sources",
                "description": "Create one source.",
                "queries": [{"query": null, "parameters": []}],
            },
        ])
    );

    let one = serde_json::to_value(catalog.select(Some("POST /sources"), None).complete()).unwrap();
    assert_eq!(
        one["routes"],
        json!([{
            "method": "post",
            "path": "/sources",
            "description": "Create one source.",
            "request_schema": {"type": "object"},
            "queries": [{
                "query": null,
                "parameters": [],
                "success_schema": {"type": "object"},
                "failure_schema": {"type": "object"},
            }],
        }])
    );
}

#[tokio::test]
#[verifies("rule_api_catalog_filter_selects_one_route", examples)]
async fn a_filter_that_is_not_the_method_and_path_of_a_route_matches_the_description() {
    let paths = |selection: CatalogSelection| {
        selection
            .complete()
            .routes
            .into_iter()
            .map(|route| route.path)
            .collect::<Vec<_>>()
    };
    // Only the descriptions of the odd routes contain this text.
    assert_eq!(
        paths(discover(6, Some("READ ONE source"), None).await),
        ["/r1", "/r3", "/r5"]
    );
    // A filter can go from the path into the description.
    assert_eq!(
        paths(discover(20, Some("/r3 read one"), None).await),
        ["/r3"]
    );
    assert_eq!(
        paths(discover(20, Some("GET /r1 list"), None).await),
        Vec::<String>::new()
    );
}

#[test]
#[verifies("rule_api_catalog_schemas_only_for_one_route", examples)]
fn the_readable_answer_of_one_route_gives_its_schemas() {
    let mut create = route(
        ApiMethod::Post,
        "/sources",
        "Create one source.",
        Some(json!({"type": "object"})),
    );
    create.queries[0].parameters = vec![parameter("Idempotency-Key", "header", true)];
    let catalog = ApiCatalog {
        routes: vec![
            create,
            route(ApiMethod::Get, "/sources", "List sources.", None),
        ],
    };

    let one = catalog
        .clone()
        .select(Some("POST /sources"), None)
        .complete();
    assert_eq!(
        render_discovery_readable(&one, SEE_MORE),
        concat!(
            "api routes: 1\n",
            "- POST /sources\n  Create one source.\n",
            "  inputs: Idempotency-Key (header, required)\n",
            "  The JSON body is the data object, without the HTTP {\"data\": ...} wrapper.\n",
            "  schemas: {\"method\":\"post\",\"path\":\"/sources\",",
            "\"description\":\"Create one source.\",\"request_schema\":{\"type\":\"object\"},",
            "\"queries\":[{\"query\":null,\"parameters\":[{\"name\":\"Idempotency-Key\",",
            "\"location\":\"header\",\"required\":true,\"schema\":{\"type\":\"string\"}}],",
            "\"success_schema\":{\"type\":\"object\"},\"failure_schema\":{\"type\":\"object\"}}]}"
        )
    );

    let both =
        render_discovery_readable(&catalog.select(Some("sources"), None).complete(), SEE_MORE);
    assert!(!both.contains("schemas:"), "{both}");
    assert!(
        both.ends_with("\nUse ROUTE to see the schemas of one route."),
        "{both}"
    );
}
