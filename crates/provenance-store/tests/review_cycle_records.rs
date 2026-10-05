#[allow(dead_code)]
mod review_support;
use provenance_core::StableId;
use provenance_macros::verifies;
use provenance_store::state_store::StateStore;
use review_support::*;
use serde_json::json;

fn pending(store: &StateStore) -> StableId {
    store
        .requirement_decision_state(&scope(), &id())
        .unwrap()
        .pending
        .unwrap()
        .proposal_id
}

fn withdraw(store: &StateStore, proposal: &StableId) {
    store
        .withdraw_record_review(
            serde_json::from_value(json!({
                "scope_id":"default", "actor":"ben", "proposal_id":proposal,
                "declared_by":null, "reason":"The text needs more work."
            }))
            .unwrap(),
        )
        .unwrap();
}

#[test]
#[verifies("rule_review_proposal_key_names_record_cycle", examples)]
fn review_proposal_key_names_record_and_cycle() {
    let (_temp, store) = fixture();
    withdraw(&store, &pending(&store));
    store
        .submit_record_review(
            serde_json::from_value(json!({
                "scope_id":"default", "actor":"ben", "record_kind":"requirement",
                "record_id":"req_a", "declared_by":null, "title":"Review again",
                "summary":"Review the record again.", "source_ids":[],
                "evidence_references":[], "builds_on":[], "expected_revision":null,
                "revises":null
            }))
            .unwrap(),
        )
        .unwrap();

    let mut keys = store
        .list_proposal_definitions(&scope())
        .unwrap()
        .into_iter()
        .map(|proposal| proposal.proposal_key)
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        ["review:requirement:req_a:1", "review:requirement:req_a:2"]
    );
}

#[test]
#[verifies("rule_withdrawal_preserves_review_history", examples)]
fn withdrawal_keeps_feedback_and_decisions() {
    let (_temp, store) = fixture();
    allow_reviewer(&store);
    let rejected = pending(&store);
    let decision = store
        .decide_record_review(
            serde_json::from_value(json!({
                "scope_id":"default", "actor":{"identity_type":"human","id":"reviewer"},
                "proposal_id":rejected, "decision":"rejected", "rationale":"Too broad.",
                "canonical_artifact":null, "declared_by":null,
                "feedback":{"role":"user","body":"Name the stored records."}
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .save_requirement(save(
            &store,
            json!({"description":"Stored records."}),
        ))
        .unwrap();
    let revised = pending(&store);

    withdraw(&store, &revised);

    let state = store.requirement_decision_state(&scope(), &id()).unwrap();
    assert!(state.pending.is_none());
    assert_eq!(state.withdrawn, std::slice::from_ref(&revised));
    assert_eq!(state.decisions.len(), 1);
    assert_eq!(
        Some(&state.decisions[0].disposition.id),
        decision.disposition_id.as_ref()
    );
    assert_eq!(
        state.decisions[0].feedback_message_id,
        decision.feedback_message_id
    );
    assert!(decision.feedback_message_id.is_some());
    assert!(store
        .list_proposal_definitions(&scope())
        .unwrap()
        .iter()
        .any(|proposal| proposal.id == revised));
}
