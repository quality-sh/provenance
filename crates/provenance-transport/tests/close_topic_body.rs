//! `POST /topics/{id}/close` takes no request body.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
use provenance_porcelain::api::{ApiArguments, ApiErrorKind, ApiMethod, ApiOutcome, ApiRequest};
use provenance_porcelain::Porcelain;
use provenance_transport::porcelain::HostApiPort;
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, host};

const CLOSE: &str = "/topics/topic_shared/close";

async fn topic_status(host: &provenance_transport::StatementHost) -> serde_json::Value {
    let (status, read) = call(host, "GET", "/topics/topic_shared", None).await;
    assert_eq!(status, 200, "{read}");
    read["data"]["status"].clone()
}

/// The close route gets the Topic from its path and the scope from the bound context, so it
/// takes no request body. A call with a body is refused and does not close the Topic.
#[tokio::test]
#[verifies("rule_route_without_body_fields_takes_no_body", examples)]
async fn close_topic_takes_no_request_body() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let open = topic_status(&host).await;

    let (status, refused) = call(&host, "POST", CLOSE, Some(json!({"data":{}}))).await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(
        refused["error"],
        json!({"kind":"invalid_input","field":null,"reason":"invalid_value"})
    );
    assert_eq!(topic_status(&host).await, open);

    let (status, closed) = call(&host, "POST", CLOSE, None).await;
    assert_eq!(status, 200, "{closed}");
    assert_eq!(closed["data"]["status"], "closed");
}

/// The api command gives the close route no input. A call with an input object is refused,
/// and the refusal names the input argument.
#[tokio::test]
#[verifies("rule_route_without_body_fields_takes_no_body", examples)]
async fn api_command_refuses_an_input_for_the_close_route() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let service = Porcelain::new(HostApiPort::new(host(&repo, true)));
    let close = |input| {
        let arguments = ApiArguments {
            path: Some(CLOSE.into()),
            method: Some(ApiMethod::Post),
            input,
            ..ApiArguments::default()
        };
        ApiRequest::from(arguments).unwrap()
    };

    let refused = service
        .execute_api(close(Some(serde_json::Map::new())))
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ApiErrorKind::InvalidOptions);
    assert_eq!(
        refused.failure["error"],
        json!({"kind":"invalid_input","field":"input","reason":"invalid_value"})
    );

    let ApiOutcome::Invoked(closed) = service.execute_api(close(None)).await.unwrap() else {
        panic!("the call runs the route");
    };
    assert_eq!(closed["data"]["status"], "closed");
}
