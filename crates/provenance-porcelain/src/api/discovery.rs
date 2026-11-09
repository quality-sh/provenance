//! Catalog discovery for each consumer: the filter and the limit, the bounds of the
//! answer, the default limit, the JSON form, and the readable summary with its "see
//! more" line.

use super::{ApiCatalog, ApiMethod, ApiParameter, ApiQuery, ApiRoute, BODY_NOTE};
use provenance_macros::rule;
use serde::Serialize;
use serde_json::Value;

/// The number of routes in a bounded catalog answer when the request gives no limit.
pub const DEFAULT_DISCOVERY_LIMIT: usize = 50;

/// How one catalog answer was bounded.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CatalogBounds {
    /// The number of routes in the catalog.
    pub total: usize,
    /// The number of routes that the filter selects.
    pub matching: usize,
    /// The number of routes in this answer.
    pub returned: usize,
    /// True when this answer has fewer routes than the filter selects.
    pub truncated: bool,
}

/// One catalog answer: the selected routes and how they were bounded. Its JSON form is
/// a [`CatalogListing`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(into = "CatalogListing")]
pub struct CatalogAnswer {
    pub routes: Vec<ApiRoute>,
    pub bounds: CatalogBounds,
}

/// The JSON form of one catalog answer.
///
/// When the answer lists one route, the route has its request and response schemas.
/// When it lists more routes, each route has its method, path, description, and
/// inputs, and no schemas, so that a reader that takes the whole answer at once can
/// read a long list.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CatalogListing {
    pub routes: Vec<ListedRoute>,
    pub bounds: CatalogBounds,
}

/// One route of a catalog listing. The schemas are present only when the listing has
/// one route.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ListedRoute {
    pub method: ApiMethod,
    pub path: String,
    pub description: String,
    /// The JSON body schema: the data object, without the HTTP
    /// `{"data": ...}` wrapper.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_schema: Option<Value>,
    pub queries: Vec<ListedQuery>,
}

/// One `query` form of a listed route.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ListedQuery {
    pub query: Option<String>,
    pub parameters: Vec<ListedParameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_schema: Option<Value>,
}

/// One parameter of a listed `query` form.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ListedParameter {
    pub name: String,
    pub location: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

impl From<CatalogAnswer> for CatalogListing {
    #[rule("rule_api_catalog_schemas_only_for_one_route")]
    fn from(answer: CatalogAnswer) -> Self {
        let schemas = answer.routes.len() == 1;
        Self {
            routes: answer
                .routes
                .into_iter()
                .map(|route| ListedRoute::new(route, schemas))
                .collect(),
            bounds: answer.bounds,
        }
    }
}

impl ListedRoute {
    fn new(route: ApiRoute, schemas: bool) -> Self {
        Self {
            method: route.method,
            path: route.path,
            description: route.description,
            request_schema: route.request_schema.filter(|_| schemas),
            queries: route
                .queries
                .into_iter()
                .map(|form| ListedQuery::new(form, schemas))
                .collect(),
        }
    }
}

impl ListedQuery {
    fn new(form: ApiQuery, schemas: bool) -> Self {
        Self {
            query: form.query,
            parameters: form
                .parameters
                .into_iter()
                .map(|parameter| ListedParameter {
                    name: parameter.name,
                    location: parameter.location,
                    required: parameter.required,
                    schema: schemas.then_some(parameter.schema),
                })
                .collect(),
            success_schema: schemas.then_some(form.success_schema),
            failure_schema: schemas.then_some(form.failure_schema),
        }
    }
}

/// The catalog routes that the filter and the limit of one request select.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSelection {
    routes: Vec<ApiRoute>,
    total: usize,
    matching: usize,
    limited: bool,
}

impl ApiCatalog {
    /// The routes whose method, path, or description contains `filter`, without letter
    /// case, and at most `limit` of them. When `filter` is the method and path of one
    /// route, such as `GET /rules`, it selects only that route.
    #[rule("rule_cli_api_discovery_filter_limit")]
    pub fn select(self, filter: Option<&str>, limit: Option<usize>) -> CatalogSelection {
        let total = self.routes.len();
        let needle = filter.map(str::to_ascii_lowercase);
        let exact = needle
            .as_deref()
            .is_some_and(|needle| names_one_route(&self.routes, needle));
        let mut routes = self
            .routes
            .into_iter()
            .filter(|route| {
                needle.as_deref().is_none_or(|needle| {
                    if exact {
                        return route_name(route) == needle;
                    }
                    format!(
                        "{} {}",
                        route_name(route),
                        route.description.to_ascii_lowercase()
                    )
                    .contains(needle)
                })
            })
            .collect::<Vec<_>>();
        let matching = routes.len();
        if let Some(limit) = limit {
            routes.truncate(limit);
        }
        CatalogSelection {
            routes,
            total,
            matching,
            limited: limit.is_some(),
        }
    }
}

/// The method and path of one route in lower case, such as `get /rules`.
fn route_name(route: &ApiRoute) -> String {
    format!("{} {}", route.method.as_str(), route.path).to_ascii_lowercase()
}

/// True when the lower-case filter is the method and path of one catalog route, so
/// that the filter selects only that route.
#[rule("rule_api_catalog_filter_selects_one_route")]
fn names_one_route(routes: &[ApiRoute], needle: &str) -> bool {
    routes.iter().any(|route| route_name(route) == needle)
}

impl CatalogSelection {
    /// Each selected route. Without a limit in the request, this is each matching route.
    pub fn complete(self) -> CatalogAnswer {
        self.answer(None)
    }

    /// At most the limit of the request, or at most [`DEFAULT_DISCOVERY_LIMIT`] routes
    /// when the request gives no limit, for a reader that takes the whole answer at once.
    #[rule("rule_porcelain_output_reports_bounds")]
    pub fn bounded(self) -> CatalogAnswer {
        let cap = (!self.limited).then_some(DEFAULT_DISCOVERY_LIMIT);
        self.answer(cap)
    }

    fn answer(mut self, cap: Option<usize>) -> CatalogAnswer {
        if let Some(cap) = cap {
            self.routes.truncate(cap);
        }
        let returned = self.routes.len();
        CatalogAnswer {
            routes: self.routes,
            bounds: CatalogBounds {
                total: self.total,
                matching: self.matching,
                returned,
                truncated: returned < self.matching,
            },
        }
    }
}

/// How one interface writes the filter and the limit in the "see more" line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeeMore {
    pub filter: &'static str,
    pub limit: &'static str,
}

impl CatalogAnswer {
    /// The line that says how to see more routes, when the answer is truncated.
    pub fn see_more(&self, fields: SeeMore) -> Option<String> {
        self.bounds
            .truncated
            .then(|| format!("Use {} or {} to see more.", fields.filter, fields.limit))
    }
}

/// Render the routes of one answer as one readable summary, with the "see more" line
/// when the answer is truncated.
///
/// Each route lists the inputs of its base form and of each `query` form;
/// the JSON form of an answer with one route has its request and response schemas.
pub fn render_discovery_readable(answer: &CatalogAnswer, fields: SeeMore) -> String {
    let mut lines = vec![format!("api routes: {}", answer.routes.len())];
    for route in &answer.routes {
        lines.push(format!(
            "- {} {}",
            route.method.as_str().to_uppercase(),
            route.path
        ));
        lines.push(format!("  {}", route.description));
        for form in &route.queries {
            let label = form.query.as_ref().map_or_else(
                || "inputs".to_owned(),
                |name| format!("inputs with query={name}"),
            );
            lines.push(format!(
                "  {label}: {}",
                render_parameters(&form.parameters)
            ));
        }
        if route.request_schema.is_some() {
            lines.push(format!("  {BODY_NOTE}"));
        }
    }
    lines.extend(answer.see_more(fields));
    lines.join("\n")
}

fn render_parameters(parameters: &[ApiParameter]) -> String {
    if parameters.is_empty() {
        return "none".to_owned();
    }
    parameters
        .iter()
        .map(|parameter| {
            let need = if parameter.required {
                "required"
            } else {
                "optional"
            };
            format!("{} ({}, {need})", parameter.name, parameter.location)
        })
        .collect::<Vec<_>>()
        .join(", ")
}
