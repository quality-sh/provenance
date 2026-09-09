use super::support::Fixture;
use serde_json::json;

#[tokio::test]
async fn native_draft_replacements_are_available_through_the_catalog() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let mut contribution = json!({"scope_id":"default","id":"contribution_one","target":{"artifact_type":"requirement","artifact_id":"req_one"},"participant_slot":"reviewer","stance":"support","strongest_finding":"Observed","evidence_references":[],"material_claims":[],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"Direct"},"open_questions":[]});
    fixture
        .call("create-contribution", contribution.clone())
        .await
        .unwrap();
    contribution["strongest_finding"] = json!("Revised");
    let updated = fixture
        .call("upsert-contribution", contribution)
        .await
        .unwrap();
    assert_eq!(updated["strongest_finding"], "Revised");
    let mut packet = json!({"scope_id":"default","id":"synthesis_one","target":{"artifact_type":"requirement","artifact_id":"req_one"},"summary":"Observed","consensus":[],"contested_claims":[],"minority_objections":[],"evidence_gaps":[],"unsupported_speculation":[],"open_questions":[],"suggested_artifacts":[],"required_human_decisions":[]});
    fixture
        .call("create-synthesis-packet", packet.clone())
        .await
        .unwrap();
    packet["summary"] = json!("Revised");
    let updated = fixture
        .call("upsert-synthesis-packet", packet)
        .await
        .unwrap();
    assert_eq!(updated["summary"], "Revised");
    assert_eq!(
        fixture
            .store
            .list_synthesis_packets(&fixture.scope)
            .unwrap()
            .len(),
        1
    );
}
