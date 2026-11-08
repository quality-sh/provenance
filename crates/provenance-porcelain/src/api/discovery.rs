//! Catalog discovery for each consumer: the filter and the limit, the bounds of the
//! answer, the default limit, and the readable summary with its "see more" line.

use super::{ApiCatalog, ApiParameter, ApiRoute, BODY_NOTE};
use provenance_macros::rule;
use serde::Serialize;

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

/// One catalog answer: the selected routes and how they were bounded.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CatalogAnswer {
    pub routes: Vec<ApiRoute>,
    pub bounds: CatalogBounds,
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
    /// case, and at most `limit` of them.
    #[rule("rule_cli_api_discovery_filter_limit")]
    pub fn select(self, filter: Option<&str>, limit: Option<usize>) -> CatalogSelection {
        let total = self.routes.len();
        let needle = filter.map(str::to_ascii_lowercase);
        let mut routes = self
            .routes
            .into_iter()
            .filter(|route| {
                needle.as_ref().is_none_or(|needle| {
                    format!(
                        "{} {} {}",
                        route.method.as_str(),
                        route.path,
                        route.description
                    )
                    .to_ascii_lowercase()
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
    #[rule("rule_porcelain_output_reports_bounds")]
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
/// the JSON form keeps the full request and response schemas.
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
