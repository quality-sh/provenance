use provenance_http_client::{Error, HttpClient};
use serde_json::{json, Value};

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs writes"]
async fn generated_authoring_methods_persist_and_preserve_declared_refusals() {
    let fixture: Value =
        serde_json::from_str(&std::env::var("PROVENANCE_RECORDS_FIXTURE").unwrap()).unwrap();
    let client =
        HttpClient::connect_with_bearer(fixture["url"].as_str().unwrap(), "fixture-secret")
            .await
            .unwrap();
    let context = json!({"repository":"fixture","scope":"default"});
    let request = json!({"schema_version":2,"spec":"rust","declared_by":"fixture-rust",
        "requirements":[{"key":"ready","statement":"The system is prepared."}],
        "rules":[{"key":"ready","requirement":"ready","statement":"The system is prepared."}]});
    let call = json!({"context":context,"request":request});
    let planned = client
        .plan(&serde_json::from_value(call.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(planned.created, 2);
    let applied = client
        .apply(&serde_json::from_value(call.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(applied.created, 2);
    let again = client
        .plan(&serde_json::from_value(call).unwrap())
        .await
        .unwrap();
    assert_eq!(again.unchanged, 2);
    let applied = serde_json::to_value(applied).unwrap();
    let rule = &applied["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap()["id"];
    let run = client.begin_verification(&serde_json::from_value(json!({"context":context,"request":{"rule":rule,"key":"rust","method":"examples","declared_by":"fixture-rust","file":"check.rs"}})).unwrap()).await.unwrap();
    let run = serde_json::to_value(run).unwrap();
    assert_eq!(run["status"], "running");
    let complete = json!({"context":context,"request":{"run":run["id"],"status":"passed"}});
    let completed = client
        .complete_verification(&serde_json::from_value(complete.clone()).unwrap())
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(completed).unwrap()["status"], "passed");
    let error = client
        .complete_verification(&serde_json::from_value(complete).unwrap())
        .await
        .unwrap_err();
    match error {
        Error::Operation { status, failure } => {
            assert_eq!(status, 409);
            assert_eq!(
                serde_json::to_value(failure).unwrap()["error"]["kind"],
                "already_complete"
            );
        }
        error => panic!("expected typed refusal, got {error:?}"),
    }
    let runs = client
        .verification_runs(
            &serde_json::from_value(json!({"context":context,"request":{"rule":rule}})).unwrap(),
        )
        .await
        .unwrap();
    assert!(serde_json::to_value(runs)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value["id"] == run["id"] && value["status"] == "passed"));
}
