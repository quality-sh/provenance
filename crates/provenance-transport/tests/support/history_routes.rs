//! The 18 history routes of the review page, called on one host for each
//! scope over the history of `history_fixture.rs`.
//!
//! The routes read the requests of the store's history cases
//! (`history_cases.rs`): each case is one route, and each request of a case
//! is one call of its route, with the request fields as path parameters and
//! query parameters.

use super::history_cases::{self, Case, Reads, CASES, ENTRY, EVIDENCE, LIST};
use super::router_answer::{Answer, Http};
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// The name in the contract of the route of `case`.
pub fn route_name(case: &Case) -> String {
    match case.operation {
        LIST => format!("list-{}-history", case.kind),
        ENTRY => format!("get-{}-history-entry", case.kind),
        EVIDENCE => format!("get-{}-history-evidence", case.kind),
        other => panic!("{other} is not a history operation"),
    }
}

/// The path of the route `name`.
pub fn route(name: &str) -> &'static str {
    provenance_contract::definitions()
        .iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("no route {name}"))
        .path
}

/// The name of each route of the contract whose path reads record
/// history.
pub fn history_routes() -> Vec<&'static str> {
    provenance_contract::definitions()
        .iter()
        .filter(|definition| definition.path.contains("/history"))
        .map(|definition| definition.name)
        .collect()
}

async fn get(http: &Http, path: String) -> Answer {
    http.call("GET", &path, None, &[], format!("GET {path}"))
        .await
}

/// The name of a path parameter segment, without its braces.
fn parameter(segment: &str) -> Option<&str> {
    segment.strip_prefix('{')?.strip_suffix('}')
}

/// The path of the call of `operation` with `request`: the route of its
/// case, with each path parameter and each other request field set.
fn path(operation: &str, request: &Value) -> String {
    let kind = request["record_kind"].as_str().unwrap();
    let case = CASES
        .iter()
        .find(|case| case.operation == operation && case.kind == kind)
        .unwrap_or_else(|| panic!("no case {operation} {kind}"));
    let text = |name: &str| {
        request.get(name).map(|value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned)
        })
    };
    let mut used = vec!["record_kind"];
    let path = route(&route_name(case))
        .split('/')
        .map(|segment| {
            let Some(name) = parameter(segment) else {
                return segment.to_owned();
            };
            let field = if name == "id" { "record_id" } else { name };
            used.push(field);
            text(field).unwrap_or_else(|| panic!("no {field} in {request}"))
        })
        .collect::<Vec<_>>()
        .join("/");
    let query: Vec<_> = request
        .as_object()
        .unwrap()
        .keys()
        .filter(|name| !used.contains(&name.as_str()))
        .map(|name| format!("{name}={}", text(name).unwrap()))
        .collect();
    if query.is_empty() {
        path
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

/// The history routes of one host for each scope, as the reads of the
/// history cases. Each read keeps the whole answer of its call.
struct Routes<'a> {
    default: &'a Http,
    broken: &'a Http,
    answers: Mutex<Vec<Answer>>,
}

impl Reads for Routes<'_> {
    fn read<'a>(
        &'a self,
        scope: &'a str,
        operation: &'static str,
        request: Value,
    ) -> Pin<Box<dyn Future<Output = Value> + Send + 'a>> {
        Box::pin(async move {
            let http = if scope == "broken" {
                self.broken
            } else {
                self.default
            };
            let path = path(operation, &request);
            let answer = get(http, path).await;
            let body = answer.json();
            let page = json!({"result": {
                "entries": body.pointer("/data/items"),
                "next_cursor": body.pointer("/meta/next_cursor"),
            }});
            self.answers.lock().unwrap().push(answer);
            page
        })
    }
}

/// The answers of each call of the history cases, in their order, on the
/// host of `default` and on the host of `broken`.
pub async fn run(default: &Http, broken: &Http) -> Vec<Answer> {
    let routes = Routes {
        default,
        broken,
        answers: Mutex::default(),
    };
    history_cases::run(&routes, &CASES).await;
    routes.answers.into_inner().unwrap()
}
