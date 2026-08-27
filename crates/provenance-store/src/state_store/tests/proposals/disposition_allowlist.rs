use super::{super::initialized_store, proposal_input};
use crate::state_store::CreateDispositionInput;
use provenance_core::{
    DispositionActor, DispositionDecision, IdentityType, PromotionState, StableId,
};

#[test]
fn scope_validation_does_not_bypass_the_manifest_when_no_allowlist_check_fires() {
    let (_dir, store, scope) = initialized_store();
    std::fs::remove_file(store.layout.manifest_path()).unwrap();

    assert!(store.validate_ideation_scope(&scope).is_err());
}

#[test]
fn scope_validation_accepts_explicit_disposition_actor_ids() {
    let (_dir, store, scope) = initialized_store();
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("ben".into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_rejected",
            "Rejected",
            PromotionState::Proposed,
        ))
        .unwrap();
    store
        .create_disposition(CreateDispositionInput {
            scope_id: scope.clone(),
            id: StableId::new("disposition_rejected").unwrap(),
            proposal_id: StableId::new("proposal_rejected").unwrap(),
            decision: DispositionDecision::Rejected,
            rationale: "Did not pass adjudication".into(),
            actor: DispositionActor {
                identity_type: IdentityType::Human,
                id: "ben".into(),
                name: None,
            },
            canonical_artifact: None,
            external_action: None,
        })
        .unwrap();

    manifest.disposition_actor_ids.clear();
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    store
        .validate_ideation_scope_with_actor_ids(&scope, &["ben".into()])
        .unwrap();
    assert!(store.validate_ideation_scope(&scope).is_err());
}
