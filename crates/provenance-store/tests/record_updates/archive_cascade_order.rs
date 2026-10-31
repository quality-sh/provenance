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

/// Creates `req_one` with an approved Resolution, and an active Rule under
/// `rule_requirement` that cites that Resolution.
async fn ruled_graph(fixture: &Fixture, rule_requirement: &str) {
    fixture.requirement().await;
    fixture.call("create-requirement-native", json!({"scope_id":"default","id":"req_two","statement":"The system keeps the record.","status":"active","depends_on":[],"supersedes":[]})).await.unwrap();
    fixture.call("create-resolution", json!({"scope_id":"default","id":"resolution_one","title":"Decision","position":"Use the saved record.","rationale":"The record is available.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();
    fixture.call("update-resolution", json!({"scope_id":"default","id":"resolution_one","status":"approved","approved_by":"reviewer","approved_at":1234})).await.unwrap();
    fixture.call("create-rule", json!({"scope_id":"default","id":"rule_one","statement":"The system saves the record.","status":"active","severity":"medium","requirement_ids":[rule_requirement],"resolution_ids":["resolution_one"]})).await.unwrap();
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
    records.extend(store.list_rules(scope).unwrap().iter().map(to_value));
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

/// The refusal text when `owner` cites the archived `resolution_one`.
fn archived_citation(owner: &str) -> String {
    format!(
        "{owner} cites resolution resolution_one, which is archived. An answered Question or an \
         active Rule can cite only an approved Resolution. Until a reviewer approves \
         resolution_one, keep the Rule in draft or review, or leave the Question open, or remove \
         the citation and name resolution_one in the text."
    )
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

/// The Resolution and the active Rule that cites it are in the same cascade, so
/// one archive command archives both.
#[tokio::test]
#[verifies("rule_archive_completes_cascade_in_one_operation", examples)]
async fn archive_cascade_archives_a_resolution_and_the_rule_it_produces() {
    let fixture = Fixture::new();
    ruled_graph(&fixture, "req_one").await;
    archive_req_one(&fixture).await.unwrap();
    assert_eq!(
        statuses(&fixture, &["req_one", "resolution_one", "rule_one"]),
        ["archived", "archived", "archived"]
    );
}

/// A live answered Question outside the cascade still cannot cite a Resolution
/// that the cascade archives, and the refusal changes no record.
// Implementation aid: this shows that the write order keeps the refusal.
// No Rule states the refusal of a live citation in an archive cascade.
#[tokio::test]
async fn archive_cascade_still_refuses_a_live_question_that_cites_an_archived_resolution() {
    let fixture = Fixture::new();
    answered_graph(&fixture, "req_two").await;
    let error = archive_req_one(&fixture).await.unwrap_err();
    assert_eq!(error["kind"], "resolution_not_approved", "{error}");
    assert_eq!(error["message"], archived_citation("question question_one"));
    assert_eq!(
        statuses(&fixture, &["req_one", "resolution_one", "question_one"]),
        ["active", "approved", "answered"]
    );
}

/// An active Rule outside the cascade still cannot cite a Resolution that the
/// cascade archives, and the refusal changes no record.
// Implementation aid, as above.
#[tokio::test]
async fn archive_cascade_still_refuses_a_live_rule_that_cites_an_archived_resolution() {
    let fixture = Fixture::new();
    ruled_graph(&fixture, "req_two").await;
    let error = archive_req_one(&fixture).await.unwrap_err();
    assert_eq!(error["kind"], "resolution_not_approved", "{error}");
    assert_eq!(error["message"], archived_citation("rule rule_one"));
    assert_eq!(
        statuses(&fixture, &["req_one", "resolution_one", "rule_one"]),
        ["active", "approved", "active"]
    );
}
