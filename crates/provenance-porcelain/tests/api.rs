#[path = "support/api_routes.rs"]
mod api_routes;

use api_routes::route;
use provenance_porcelain::api::{
    ApiArguments, ApiCatalog, ApiError, ApiErrorKind, ApiInput, ApiMethod, ApiOutcome, ApiPort,
    ApiPortFuture, ApiRequest,
};
use provenance_porcelain::Porcelain;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingPort {
    seen: Arc<Mutex<Option<ApiInput>>>,
    result: Value,
    refusal: Option<ApiError>,
}

impl RecordingPort {
    fn unknown_path() -> ApiError {
        ApiError {
            kind: ApiErrorKind::UnknownPath,
            failure: json!({"error": {"kind": "unknown_operation"}, "meta": {}}),
            cause: None,
        }
    }
}

impl ApiPort for RecordingPort {
    fn invoke(&self, input: ApiInput) -> ApiPortFuture<'_, Value> {
        *self.seen.lock().unwrap() = Some(input);
        let result = self.result.clone();
        let refusal = self.refusal.clone();
        Box::pin(async move { refusal.map_or(Ok(result), Err) })
    }

    fn discover(&self) -> ApiCatalog {
        ApiCatalog {
            routes: vec![route(
                ApiMethod::Get,
                "/requirements",
                "List requirements.",
                None,
            )],
        }
    }
}

fn body(value: &Value) -> Option<serde_json::Map<String, Value>> {
    value.as_object().cloned()
}

fn arguments(path: &str) -> ApiArguments {
    ApiArguments {
        path: Some(path.into()),
        ..Default::default()
    }
}

fn invoke(path: &str) -> ApiRequest {
    ApiRequest::Invoke(ApiInput {
        method: ApiMethod::Get,
        path: path.into(),
        query: BTreeMap::new(),
        headers: BTreeMap::new(),
        body: None,
    })
}

#[test]
fn api_arguments_default_to_get_on_the_given_path() {
    let ApiRequest::Invoke(input) =
        ApiRequest::from(arguments("/requirements")).expect("valid request")
    else {
        panic!("a path selects one invocation");
    };
    assert_eq!(input.method, ApiMethod::Get);
    assert_eq!(input.path, "/requirements");
    assert!(input.query.is_empty());
    assert!(input.headers.is_empty());
    assert!(input.body.is_none());
}

#[test]
fn api_arguments_select_methods_bodies_queries_and_headers() {
    let arguments = ApiArguments {
        path: Some("/requirements".into()),
        method: Some(ApiMethod::Post),
        query: BTreeMap::from([("limit".into(), "5".into())]),
        header: BTreeMap::from([("Idempotency-Key".into(), "key-one".into())]),
        input: body(&json!({"statement": "One statement."})),
        ..Default::default()
    };
    let ApiRequest::Invoke(input) = ApiRequest::from(arguments).expect("valid request") else {
        panic!("a path selects one invocation");
    };
    assert_eq!(input.method, ApiMethod::Post);
    assert_eq!(input.query["limit"], "5");
    assert_eq!(input.headers["Idempotency-Key"], "key-one");
    assert_eq!(input.body.unwrap()["statement"], "One statement.");
}

#[test]
fn arguments_without_a_path_select_discovery() {
    assert!(matches!(
        ApiRequest::from(ApiArguments::default()),
        Ok(ApiRequest::Discover {
            filter: None,
            limit: None
        })
    ));
}

#[test]
fn arguments_refuse_call_options_without_a_path() {
    let refused_field = |arguments: ApiArguments| {
        let error = ApiRequest::from(arguments).expect_err("call options need a path");
        assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
        assert_eq!(error.failure["error"]["kind"], "invalid_input");
        error.failure["error"]["field"].clone()
    };
    assert_eq!(
        refused_field(ApiArguments {
            method: Some(ApiMethod::Patch),
            ..Default::default()
        }),
        "method"
    );
    assert_eq!(
        refused_field(ApiArguments {
            query: BTreeMap::from([("limit".into(), "1".into())]),
            ..Default::default()
        }),
        "query"
    );
    assert_eq!(
        refused_field(ApiArguments {
            header: BTreeMap::from([("Accept".into(), "json".into())]),
            ..Default::default()
        }),
        "header"
    );
    assert_eq!(
        refused_field(ApiArguments {
            input: body(&json!({})),
            ..Default::default()
        }),
        "input"
    );
}

#[test]
fn methods_parse_in_any_letter_case() {
    for (name, method) in [
        ("get", ApiMethod::Get),
        ("GET", ApiMethod::Get),
        ("Post", ApiMethod::Post),
        ("PATCH", ApiMethod::Patch),
    ] {
        assert_eq!(ApiMethod::parse(name), Some(method), "{name}");
    }
    assert_eq!(ApiMethod::parse("DELETE"), None);
}

#[test]
fn header_names_that_differ_only_in_case_refuse() {
    let arguments = ApiArguments {
        path: Some("requirements/req_one".into()),
        method: Some(ApiMethod::Patch),
        header: BTreeMap::from([
            ("If-Match".into(), "\"1\"".into()),
            ("if-match".into(), "\"2\"".into()),
        ]),
        input: body(&json!({"description": "One description."})),
        ..Default::default()
    };
    let error = ApiRequest::from(arguments).expect_err("one header name, two values");
    assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
    assert_eq!(error.failure["error"]["kind"], "invalid_input");
    assert_eq!(error.failure["error"]["field"], "header");
}

#[tokio::test]
async fn api_refuses_an_empty_path_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    for path in ["", "/"] {
        let error = service.execute_api(invoke(path)).await.unwrap_err();
        assert_eq!(error.kind, ApiErrorKind::InvalidOptions, "{path}");
        assert_eq!(error.failure["error"]["field"], "path", "{path}");
    }
    assert!(port.seen.lock().unwrap().is_none());
}

#[tokio::test]
async fn api_refuses_paths_the_http_router_cannot_match() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    for path in [
        "///",
        "//requirements",
        "requirements/",
        "/requirements/req_one/",
        "requirements//req_one",
    ] {
        let error = service.execute_api(invoke(path)).await.unwrap_err();
        assert_eq!(error.kind, ApiErrorKind::UnknownPath, "{path}");
        assert_eq!(
            error.failure["error"]["kind"], "unknown_operation",
            "{path}"
        );
    }
    assert!(port.seen.lock().unwrap().is_none());
}

#[tokio::test]
async fn api_refuses_a_query_carrying_path_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let error = service
        .execute_api(invoke("/requirements?limit=1"))
        .await
        .unwrap_err();

    assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
    assert_eq!(error.failure["error"]["field"], "path");
    assert!(port.seen.lock().unwrap().is_none());
}

#[tokio::test]
async fn api_refuses_a_get_body_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let request = ApiRequest::Invoke(ApiInput {
        method: ApiMethod::Get,
        path: "requirements".into(),
        query: BTreeMap::new(),
        headers: BTreeMap::new(),
        body: body(&json!({"limit": 1})),
    });

    let error = service.execute_api(request).await.unwrap_err();

    assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
    assert_eq!(error.failure["error"]["field"], "input");
    assert!(port.seen.lock().unwrap().is_none());
}

#[tokio::test]
async fn invalid_options_carries_the_canonical_refusal_envelope() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let error = service.execute_api(invoke("")).await.unwrap_err();

    assert_eq!(error.failure["error"]["kind"], "invalid_input");
    assert_eq!(error.failure["error"]["reason"], "invalid_value");
    assert_eq!(error.failure["error"]["field"], "path");
    assert!(error.failure["meta"].is_object());
}

#[tokio::test]
async fn api_sends_the_normalized_input_to_the_port_and_returns_its_value() {
    let port = RecordingPort {
        result: json!({"data": {"id": "req_one"}, "meta": {}}),
        ..Default::default()
    };
    let service = Porcelain::new(port.clone());

    let outcome = service
        .execute_api(ApiRequest::from(arguments("requirements/req_one")).expect("valid request"))
        .await
        .unwrap();

    let sent = port.seen.lock().unwrap().clone().unwrap();
    assert_eq!(sent.path, "/requirements/req_one");
    assert_eq!(
        outcome,
        ApiOutcome::Invoked(json!({"data": {"id": "req_one"}, "meta": {}}))
    );
}

#[tokio::test]
async fn api_forwards_the_port_refusal() {
    let port = RecordingPort {
        refusal: Some(RecordingPort::unknown_path()),
        ..Default::default()
    };
    let service = Porcelain::new(port.clone());
    let expected = RecordingPort::unknown_path();

    let error = service
        .execute_api(ApiRequest::from(arguments("sources/source_missing")).expect("valid request"))
        .await
        .unwrap_err();

    assert_eq!(error, expected);
}

#[tokio::test]
async fn discovery_returns_the_port_catalog() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let outcome = service
        .execute_api(ApiRequest::Discover {
            filter: None,
            limit: None,
        })
        .await
        .unwrap();

    let ApiOutcome::Catalog(selection) = outcome else {
        panic!("discovery returns the catalog");
    };
    let catalog = selection.complete();
    assert_eq!(catalog.routes.len(), 1);
    assert_eq!(catalog.routes[0].path, "/requirements");
    assert_eq!(catalog.routes[0].method, ApiMethod::Get);
}

#[test]
fn each_api_error_kind_displays_its_own_message() {
    for (kind, message) in [
        (ApiErrorKind::InvalidOptions, "unsupported api options"),
        (ApiErrorKind::UnknownPath, "unknown api path"),
        (
            ApiErrorKind::MethodNotAllowed,
            "method not allowed for this path",
        ),
        (ApiErrorKind::AccessDenied, "api access denied"),
        (ApiErrorKind::Operation, "api operation failed"),
    ] {
        let error = ApiError {
            kind,
            failure: Value::Null,
            cause: None,
        };
        assert_eq!(error.to_string(), message, "{kind:?}");
    }
}
