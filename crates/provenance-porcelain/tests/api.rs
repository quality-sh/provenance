use provenance_porcelain::api::{
    render_discovery_readable, ApiArguments, ApiCatalog, ApiError, ApiErrorKind, ApiInput,
    ApiMethod, ApiOutcome, ApiPort, ApiRequest, ApiRoute, ApiPortFuture,
};
use provenance_porcelain::Porcelain;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingPort {
    seen: Arc<Mutex<Option<ApiInput>>>,
    result: Value,
}

impl RecordingPort {
    fn unknown_path() -> ApiError {
        ApiError {
            kind: ApiErrorKind::UnknownPath,
            failure: json!({"error": {"kind": "unknown_operation"}, "meta": {}}),
        }
    }
}

impl ApiPort for RecordingPort {
    fn invoke(&self, input: ApiInput) -> ApiPortFuture<'_, Value> {
        *self.seen.lock().unwrap() = Some(input.clone());
        let result = self.result.clone();
        Box::pin(async move { Ok(result) })
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

fn route(
    method: ApiMethod,
    path: &str,
    description: &str,
    request_schema: Option<Value>,
) -> ApiRoute {
    ApiRoute {
        method,
        path: path.into(),
        description: description.into(),
        parameters: Vec::new(),
        request_schema,
        response_schema: json!({"type": "object"}),
    }
}

fn body(value: Value) -> Option<serde_json::Map<String, Value>> {
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
    assert_eq!(input.path, "requirements");
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
        headers: BTreeMap::from([("Idempotency-Key".into(), "key-one".into())]),
        body: body(json!({"statement": "One statement."})),
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
        Ok(ApiRequest::Discover)
    ));
}

#[test]
fn arguments_refuse_call_options_without_a_path() {
    let without_path = |mut arguments: ApiArguments| {
        arguments.path = None;
        ApiRequest::from(arguments)
            .expect_err("call options need a path")
            .kind
    };
    assert_eq!(
        without_path(ApiArguments {
            method: Some(ApiMethod::Patch),
            ..Default::default()
        }),
        ApiErrorKind::InvalidOptions
    );
    assert_eq!(
        without_path(ApiArguments {
            query: BTreeMap::from([("limit".into(), "1".into())]),
            ..Default::default()
        }),
        ApiErrorKind::InvalidOptions
    );
    assert_eq!(
        without_path(ApiArguments {
            headers: BTreeMap::from([("Accept".into(), "json".into())]),
            ..Default::default()
        }),
        ApiErrorKind::InvalidOptions
    );
    assert_eq!(
        without_path(ApiArguments {
            body: body(json!({})),
            ..Default::default()
        }),
        ApiErrorKind::InvalidOptions
    );
}

#[tokio::test]
async fn api_refuses_an_empty_path_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let error = service.execute_api(invoke("///")).await.unwrap_err();

    assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
    assert!(port.seen.lock().unwrap().is_none());
}

#[tokio::test]
async fn api_refuses_a_query_carrying_path_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let error = service.execute_api(invoke("/requirements?limit=1")).await.unwrap_err();

    assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
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
        body: body(json!({"limit": 1})),
    });

    let error = service.execute_api(request).await.unwrap_err();

    assert_eq!(error.kind, ApiErrorKind::InvalidOptions);
    assert!(port.seen.lock().unwrap().is_none());
}

#[tokio::test]
async fn invalid_options_carries_the_canonical_refusal_envelope() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let error = service.execute_api(invoke("")).await.unwrap_err();

    assert_eq!(error.failure["error"]["kind"], "invalid_input");
    assert_eq!(error.failure["error"]["reason"], "invalid_value");
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
        .execute_api(ApiRequest::from(arguments("/requirements/req_one/")).expect("valid request"))
        .await
        .unwrap();

    let sent = port.seen.lock().unwrap().clone().unwrap();
    assert_eq!(sent.path, "requirements/req_one");
    assert_eq!(
        outcome,
        ApiOutcome::Invoked(json!({"data": {"id": "req_one"}, "meta": {}}))
    );
}

#[tokio::test]
async fn api_forwards_the_port_refusal() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let expected = RecordingPort::unknown_path();

    let error = service
        .execute_api(
            ApiRequest::from(arguments("sources/source_missing")).expect("valid request"),
        )
        .await
        .unwrap_err();

    assert_eq!(error, expected);
}

#[tokio::test]
async fn discovery_returns_the_port_catalog() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());

    let outcome = service.execute_api(ApiRequest::Discover).await.unwrap();

    let ApiOutcome::Catalog(catalog) = outcome else {
        panic!("discovery returns the catalog");
    };
    assert_eq!(catalog.routes.len(), 1);
    assert_eq!(catalog.routes[0].path, "/requirements");
    assert_eq!(catalog.routes[0].method, ApiMethod::Get);
}

#[test]
fn readable_discovery_lists_method_path_and_description() {
    let catalog = ApiCatalog {
        routes: vec![
            route(ApiMethod::Get, "/requirements", "List requirements.", None),
            route(
                ApiMethod::Post,
                "/requirements",
                "Create one requirement.",
                Some(json!({"type": "object"})),
            ),
        ],
    };

    assert_eq!(
        render_discovery_readable(&catalog),
        concat!(
            "api routes: 2\n",
            "- GET /requirements\n  List requirements.\n",
            "- POST /requirements\n  Create one requirement.\n  The request carries one JSON body."
        )
    );
}
