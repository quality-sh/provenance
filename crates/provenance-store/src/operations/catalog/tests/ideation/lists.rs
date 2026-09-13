//! The reads return the complete native arrays and the effective-state
//! projection; a supported assertion lands through the native writer.
use super::super::super::{
    invoke_typed, CreateAssertion, CreateProposal, ListAssertions, ListDispositions, ListProposals,
};
use super::*;
use provenance_core::PromotionState;

#[tokio::test]
async fn create_proposal_matches_the_native_writer_and_persists() {
    let (_native_dir, native_store, native_scope, dir, store, scope) = twin();
    seed_requirement(&native_store, &native_scope);
    seed_requirement(&store, &scope);
    let expected = native_store
        .create_proposal_card(proposal_input(&native_scope, "proposal_overtime"))
        .unwrap();
    let actual = invoke_typed::<CreateProposal>(
        prepared(&dir, &scope),
        proposal_input(&scope, "proposal_overtime"),
    )
    .await
    .unwrap();
    assert_eq!(json!(actual), json!(expected));
    assert_eq!(
        json!(store.list_proposal_definitions(&scope).unwrap()),
        json!(native_store
            .list_proposal_definitions(&native_scope)
            .unwrap())
    );
}

#[tokio::test]
async fn ideation_lists_return_the_complete_native_arrays_and_the_effective_state() {
    let (_native_dir, native_store, native_scope, dir, store, scope) = twin();
    for (target, target_scope) in [(&native_store, &native_scope), (&store, &scope)] {
        seed_requirement(target, target_scope);
        allow_actor(target, "reviewer");
        target
            .create_proposal_card(proposal_input(target_scope, "proposal_overtime"))
            .unwrap();
        target
            .create_disposition(rejected_disposition(
                target_scope,
                "disposition_one",
                "proposal_overtime",
            ))
            .unwrap();
    }
    let expected = (
        native_store.list_proposal_cards(&native_scope).unwrap(),
        native_store.list_dispositions(&native_scope).unwrap(),
        native_store.list_assertion_records(&native_scope).unwrap(),
    );
    assert_eq!(expected.0[0].promotion_state, PromotionState::Rejected);
    let listed_proposals = invoke_typed::<ListProposals>(prepared(&dir, &scope), ())
        .await
        .unwrap();
    let listed_dispositions = invoke_typed::<ListDispositions>(prepared(&dir, &scope), ())
        .await
        .unwrap();
    let listed_assertions = invoke_typed::<ListAssertions>(prepared(&dir, &scope), ())
        .await
        .unwrap();
    assert_eq!(json!(listed_proposals), json!(expected.0));
    assert_eq!(json!(listed_dispositions), json!(expected.1));
    assert_eq!(json!(listed_assertions), json!(expected.2));
    // The projection reports the disposition's verdict; the stored definition
    // keeps the state its author wrote.
    assert_eq!(
        store.list_proposal_definitions(&scope).unwrap()[0].promotion_state,
        PromotionState::Proposed
    );
}

#[tokio::test]
async fn a_supported_assertion_lands_and_the_projection_reports_asserted() {
    let (dir, store, scope) = initialized();
    seed_blocked_evidence(&store, &scope);
    let mut proposal = proposal_input(&scope, "proposal_overtime");
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    invoke_typed::<CreateProposal>(prepared(&dir, &scope), proposal)
        .await
        .unwrap();
    qualify_seeded_evidence(&store, &scope);
    let asserted =
        invoke_typed::<CreateAssertion>(prepared(&dir, &scope), supported_assertion(&scope))
            .await
            .unwrap();
    assert_eq!(asserted.proposal_id.as_str(), "proposal_overtime");
    let listed = invoke_typed::<ListAssertions>(prepared(&dir, &scope), ())
        .await
        .unwrap();
    assert_eq!(
        json!(listed),
        json!(store.list_assertion_records(&scope).unwrap())
    );
    let proposals = invoke_typed::<ListProposals>(prepared(&dir, &scope), ())
        .await
        .unwrap();
    assert_eq!(proposals[0].promotion_state, PromotionState::Asserted);
}
