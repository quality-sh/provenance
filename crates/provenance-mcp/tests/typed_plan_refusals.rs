//! The api tool plan of a typed authoring document shows the deletion that
//! the apply refuses, with its cause.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::api_fixture::{write_host, Repository};
use support::api_session::ApiSession;

fn spec(statement: &str) -> Value {
    json!({"spec": "fx", "declared_by": "spec://fx", "sources": [],
           "requirements": [{"key": "restated", "statement": statement}]})
}

async fn authoring(session: &ApiSession, path: &str, input: Value) -> Value {
    let result = session
        .call(json!({"path": path, "method": "post", "input": input}))
        .await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let text = &result.content[0].as_text().unwrap().text;
    let structured = result.structured_content.unwrap();
    assert_eq!(serde_json::from_str::<Value>(text).unwrap(), structured);
    structured
}

#[tokio::test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
async fn the_api_tool_plan_shows_the_refused_deletion_and_its_cause() {
    let repository = Repository::new("The shared graph is readable.");
    let session = ApiSession::start(write_host(&repository)).await;
    authoring(&session, "authoring-changes", spec("The host stops.")).await;
    // The second statement is a typed change, so the apply opens a submission.
    let applied = authoring(&session, "authoring-changes", spec("The host starts.")).await;
    let id = applied["data"]["resources"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let empty = json!({"spec": "fx", "declared_by": "spec://fx", "sources": [],
                       "requirements": []});
    let message = format!(
        "cannot delete requirement {id} because it has a review submission; \
         keep the requirement in the typed document"
    );

    let plan = authoring(&session, "authoring-plans", empty).await;

    assert_eq!(
        plan["data"]["resources"][0],
        json!({"kind": "requirement", "key": "restated",
               "address": ["fx", "requirement", "restated"], "id": id, "state": "refused",
               "refusal": {"cause": "review_submission", "message": message}}),
        "{plan}"
    );
    assert_eq!(
        [&plan["data"]["deleted"], &plan["data"]["refused"]],
        [&json!(0), &json!(1)],
        "{plan}"
    );
    session.shutdown().await;
}
