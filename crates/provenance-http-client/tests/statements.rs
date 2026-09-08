use provenance_http_client::types::CheckStatementRequestInput;
use provenance_http_client::{Error, HttpClient};
use serde_json::json;

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs statements"]
async fn generated_client_matches_real_host_and_preserves_typed_refusal() {
    let url = std::env::var("PROVENANCE_TEST_HOST").expect("test fixture host URL");
    let client = HttpClient::connect(&url).await.unwrap();
    for statement in ["Install the cover.", "Stop; wait.", "Café; stop."] {
        let body = json!({"request":{"statement":statement}});
        let call: CheckStatementRequestInput = serde_json::from_value(body.clone()).unwrap();
        let actual = client.check_statement(&call).await.unwrap();
        let expected: serde_json::Value = reqwest::Client::new()
            .post(format!(
                "{url}/v{}/operations/check-statement",
                provenance_http_client::PROTOCOL_VERSION
            ))
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(serde_json::to_value(actual).unwrap(), expected);
    }
    let oversized =
        serde_json::from_value(json!({"request":{"statement":"x".repeat(2 * 1024 * 1024)}}))
            .unwrap();
    match client.check_statement(&oversized).await.unwrap_err() {
        Error::Operation { status, failure } => {
            assert_eq!(status, 400);
            let value = serde_json::to_value(failure).unwrap();
            assert_eq!(value["operation"], "check-statement");
            assert_eq!(value["error"]["kind"], "invalid_input");
            assert_eq!(value["error"]["reason"], "too_large");
        }
        error => panic!("expected typed body-size refusal, received {error}"),
    }
    assert!(matches!(
        HttpClient::connect("file:///tmp").await,
        Err(Error::InvalidUrl)
    ));
}
