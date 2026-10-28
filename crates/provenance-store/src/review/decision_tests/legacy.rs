use super::*;

#[test]
fn legacy_unbound_decisions_read_correctly_and_stay_frozen() {
    let (_temp, store, _, _) = enrolled();
    store.create_proposal_card(serde_json::from_value(json!({
        "scope_id":"default","id":"prop-legacy","proposal_key":"legacy","proposal_type":"requirement_candidate",
        "title":"Legacy","summary":"Summary","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_a"},
        "source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"builds_on":[],
        "promotion_state":"proposed"})).unwrap()).unwrap();
    store
        .create_disposition(serde_json::from_value(json!({
            "scope_id":"default","id":"disp-legacy","proposal_id":"prop-legacy","decision":"accepted",
            "rationale":"Ratified offline","actor":{"identity_type":"human","id":"reviewer"},
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"}})).unwrap())
        .unwrap();

    let recorded = state(&store);
    assert_eq!(recorded.decisions.len(), 1);
    assert!(
        recorded.decisions[0].revision.is_none(),
        "a legacy disposition binds no revision"
    );
    assert!(
        recorded.current_acceptance.is_none(),
        "a legacy acceptance attests no current content"
    );

    edit(&store, "edit-2", "Revised statement");
    assert!(
        state(&store).current_acceptance.is_none(),
        "editing keeps historical acceptance only"
    );

    refused(
        store.create_disposition(serde_json::from_value(json!({
            "scope_id":"default","id":"disp-legacy-2","proposal_id":"prop-legacy","decision":"rejected",
            "rationale":"Rewriting history","actor":{"identity_type":"human","id":"reviewer"}})).unwrap()),
        "authoritative disposition",
    );
    assert_eq!(store.list_dispositions(&scope()).unwrap().len(), 1);
}
