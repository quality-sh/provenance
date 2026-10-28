use super::support::Fixture;
use provenance_macros::verifies;
use serde_json::{json, Value};

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Creates `req_one` with an approved Resolution, and a Topic under
/// `topic_requirement` whose Question that Resolution answers.
async fn answered_graph(fixture: &Fixture, topic_requirement: &str) {
    fixture.requirement().await;
    fixture.call("create-requirement-native", json!({"scope_id":"default","id":"req_two","statement":"The system keeps the record.","status":"active","depends_on":[],"supersedes":[]})).await.unwrap();
    fixture.call("create-topic", json!({"scope_id":"default","id":"topic_one","requirement_id":topic_requirement,"title":"Topic","status":"open","links":[]})).await.unwrap();
    fixture.call("create-question", json!({"scope_id":"default","id":"question_one","topic_id":"topic_one","question":"Which record?","resolution_method":"research","status":"open","links":[]})).await.unwrap();
    fixture.call("create-resolution", json!({"scope_id":"default","id":"resolution_one","title":"Decision","position":"Use the saved record.","rationale":"The record is available.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();
    fixture.call("update-resolution", json!({"scope_id":"default","id":"resolution_one","status":"approved","approved_by":"reviewer","approved_at":1234})).await.unwrap();
    fixture.call("answer-question", json!({"scope_id":"default","id":"question_one","answer":"The saved record.","resolution_id":"resolution_one"})).await.unwrap();
}

async fn archive_req_one(fixture: &Fixture) -> Result<Value, Value> {
    fixture
        .call(
            "update-requirement-native",
            json!({"scope_id":"default","id":"req_one","status":"archived","archived_in_commit":{"commit":COMMIT}}),
        )
        .await
}

/// The stored status of each record, in the order of `ids`.
fn statuses(fixture: &Fixture, ids: &[&str]) -> Vec<String> {
    let store = &fixture.store;
    let scope = &fixture.scope;
    let mut records = Vec::new();
    records.extend(store.list_requirements(scope).unwrap().iter().map(to_value));
    records.extend(store.list_resolutions(scope).unwrap().iter().map(to_value));
    records.extend(store.list_topics(scope).unwrap().iter().map(to_value));
    records.extend(store.list_questions(scope).unwrap().iter().map(to_value));
    ids.iter()
        .map(|id| {
            let record = records.iter().find(|record| record["id"] == *id).unwrap();
            record["status"].as_str().unwrap().to_owned()
        })
        .collect()
}

fn to_value(record: &impl serde::Serialize) -> Value {
    serde_json::to_value(record).unwrap()
}

/// The Resolution and the Question that it answers are in the same cascade, so
/// one archive command archives both.
#[tokio::test]
#[verifies("rule_archive_completes_cascade_in_one_operation", examples)]
async fn archive_cascade_archives_a_resolution_and_the_question_it_answers() {
    let fixture = Fixture::new();
    answered_graph(&fixture, "req_one").await;
    archive_req_one(&fixture).await.unwrap();
    assert_eq!(
        statuses(
            &fixture,
            &["req_one", "resolution_one", "topic_one", "question_one"]
        ),
        ["archived", "archived", "archived", "archived"]
    );
}

/// A live answered Question outside the cascade still cannot cite a Resolution
/// that the cascade archives, and the refusal changes no record.
// Implementation aid: this shows that the new write order keeps the refusal.
// No Rule states the refusal of a live citation in an archive cascade.
#[tokio::test]
async fn archive_cascade_still_refuses_a_live_question_that_cites_an_archived_resolution() {
    let fixture = Fixture::new();
    answered_graph(&fixture, "req_two").await;
    let error = archive_req_one(&fixture).await.unwrap_err();
    assert_eq!(error["kind"], "resolution_not_approved", "{error}");
    assert_eq!(
        statuses(&fixture, &["req_one", "resolution_one", "question_one"]),
        ["active", "approved", "answered"]
    );
}
