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
    assert!(render_discovery_readable(&under_default, SEE_MORE).ends_with("inputs: none"));
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
            "  The JSON body is the data object, without the HTTP {\"data\": ...} wrapper."
        )
    );
    let limited = render_discovery_readable(&catalog.select(None, Some(2)).complete(), SEE_MORE);
    assert!(
        limited.ends_with("depth (query, optional)\nUse FILTER or LIMIT to see more."),
        "{limited}"
    );
}
