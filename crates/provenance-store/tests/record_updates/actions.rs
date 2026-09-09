use super::support::Fixture;
use serde_json::json;

#[tokio::test]
async fn relationship_operations_keep_required_relations_and_reject_cycles() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture.call("create-requirement", json!({"scope_id":"default","id":"req_two","statement":"The system reads the record.","status":"active","depends_on":[],"supersedes":[]})).await.unwrap();
    let linked = fixture
        .call(
            "add-requirement-depends-on",
            json!({"scope_id":"default","id":"req_one","target_id":"req_two"}),
        )
        .await
        .unwrap();
    assert_eq!(linked["depends_on"], json!(["req_two"]));
    assert!(fixture
        .call(
            "add-requirement-depends-on",
            json!({"scope_id":"default","id":"req_two","target_id":"req_one"})
        )
        .await
        .is_err());
    let cleared = fixture
        .call(
            "clear-requirement-depends-on",
            json!({"scope_id":"default","id":"req_one","target_id":"req_two"}),
        )
        .await
        .unwrap();
    assert!(cleared.get("depends_on").is_none());
}

#[tokio::test]
async fn shaping_actions_preserve_claim_and_answer_rules() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture.call("create-topic", json!({"scope_id":"default","id":"topic_one","requirement_id":"req_one","title":"Topic","status":"open","links":[]})).await.unwrap();
    fixture.call("create-question", json!({"scope_id":"default","id":"question_one","topic_id":"topic_one","question":"Which record?","resolution_method":"research","status":"open","links":[]})).await.unwrap();
    let claimed = fixture
        .call(
            "claim-question",
            json!({"scope_id":"default","id":"question_one","actor":"worker"}),
        )
        .await
        .unwrap();
    assert_eq!(claimed["claimed_by"], "worker");
    assert!(fixture
        .call(
            "update-question",
            json!({"scope_id":"default","id":"question_one","status":"answered"})
        )
        .await
        .is_err());
    let answered = fixture
        .call(
            "answer-question",
            json!({"scope_id":"default","id":"question_one","answer":"The saved record."}),
        )
        .await
        .unwrap();
    assert_eq!(answered["status"], "answered");
    let refusal = fixture
        .call(
            "claim-question",
            json!({"scope_id":"default","id":"question_one","actor":"worker"}),
        )
        .await
        .unwrap_err();
    assert_eq!(refusal["kind"], "invalid_update");
    assert!(answered.get("claimed_by").is_none());
    let claimed = fixture
        .call(
            "claim-topic",
            json!({"scope_id":"default","id":"topic_one","actor":"worker"}),
        )
        .await
        .unwrap();
    assert_eq!(claimed["claimed_by"], "worker");
    let closed = fixture
        .call(
            "close-topic",
            json!({"scope_id":"default","id":"topic_one"}),
        )
        .await
        .unwrap();
    assert_eq!(closed["status"], "closed");
    assert!(closed.get("claimed_by").is_none());
}

#[tokio::test]
async fn relation_and_shaping_refusals_retain_typed_causes() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let error = fixture
        .call(
            "add-requirement-depends-on",
            json!({"scope_id":"default","id":"req_one","target_id":"req_one"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error["kind"], "invalid_update");
    let error = fixture
        .call(
            "release-question",
            json!({"scope_id":"default","id":"question_missing"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error["kind"], "missing_reference");
}
