//! Catalog routes for the api tests: one route with one base form, and its parts.

// Each test file uses a part of these helpers.
#![allow(dead_code)]

use provenance_porcelain::api::{ApiMethod, ApiParameter, ApiQuery, ApiRoute};
use serde_json::{json, Value};

pub fn route(
    method: ApiMethod,
    path: &str,
    description: &str,
    request_schema: Option<Value>,
) -> ApiRoute {
    ApiRoute {
        method,
        path: path.into(),
        description: description.into(),
        request_schema,
        queries: vec![form(None)],
    }
}

pub fn form(query: Option<&str>) -> ApiQuery {
    ApiQuery {
        query: query.map(str::to_owned),
        parameters: Vec::new(),
        success_schema: json!({"type": "object"}),
        failure_schema: json!({"type": "object"}),
    }
}

pub fn parameter(name: &str, location: &str, required: bool) -> ApiParameter {
    ApiParameter {
        name: name.into(),
        location: location.into(),
        required,
        schema: json!({"type": "string"}),
    }
}
