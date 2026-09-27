use super::{super::initialized_store, proposal_input};
use crate::state_store::{CreateDispositionInput, ProposalCard, ProposalDemand, StateStore};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    DispositionActor, DispositionDecision, IdeationTarget, IdeationTargetType, IdentityType,
    PromotionState, ScopeId, StableId,
};
use provenance_macros::verifies;

#[test]
fn proposal_projection_rejects_unlisted_disposition_actor() {
    let (_dir, store, scope) = initialized_store();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::dispositions_path(&store.layout, &scope),
        &[provenance_core::DispositionRecord {
            schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
            scope_id: scope.clone(),
            id: StableId::new("disposition_overtime").unwrap(),
            proposal_id: StableId::new("proposal_overtime").unwrap(),
            decision: DispositionDecision::Accepted,
            rationale: "Forged review".into(),
            actor: disposition_actor("forged-reviewer"),
            canonical_artifact: None,
            external_action: None,
        }],
    )
    .unwrap();

    let error = store.list_proposal_cards(&scope).unwrap_err().to_string();
    assert!(
        error.contains("no disposition actors configured"),
        "{error}"
    );
}

#[test]
fn proposal_projection_uses_one_publication_snapshot() {
    let (_dir, store, scope) = initialized_store();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    let (validated_tx, validated_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let reader = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store.project_proposal_cards(&scope, None, || {
                validated_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
        })
    };
    validated_rx.recv().unwrap();

    let (published_tx, published_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|| {
                    crate::jsonl::write_jsonl_atomic::<ProposalCard>(
                        &crate::shards::proposal_cards_path(&store.layout, &scope),
                        &[],
                    )
                })
                .unwrap();
            published_tx.send(()).unwrap();
        })
    };

    assert!(published_rx
        .recv_timeout(std::time::Duration::from_millis(100))
        .is_err());
    release_tx.send(()).unwrap();
    let projected = reader.join().unwrap().unwrap();
    publisher.join().unwrap();

    assert_eq!(projected.len(), 1);
    assert!(store.list_proposal_cards(&scope).unwrap().is_empty());
}

#[test]
fn modern_proposal_rejects_duplicate_create() {
    let (_dir, store, scope) = initialized_store();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Original",
            PromotionState::Proposed,
        ))
        .unwrap();

    let error = store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Divergent",
            PromotionState::Proposed,
        ))
        .unwrap_err()
        .to_string();
    assert!(error.contains("immutable"), "{error}");
}

#[test]
#[verifies("rule_proposal_surfacing", examples)]
fn ratification_records_a_disposition_without_mutating_proposal_definition() {
    let (_dir, store, scope) = initialized_store();
    seed_proposal_ready_for_disposition(&store, &scope);
    let demand = ProposalDemand::for_target(IdeationTarget {
        artifact_type: IdeationTargetType::Requirement,
        artifact_id: StableId::new("req_overtime").unwrap(),
    });
    assert_eq!(
        store.surface_proposals(&scope, &demand).unwrap()[0]
            .proposal
            .promotion_state,
        PromotionState::Asserted
    );
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut threads = Vec::new();
    for id in ["disposition_overtime_a", "disposition_overtime_b"] {
        let store = store.clone();
        let scope = scope.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            store.create_disposition(disposition_input(scope, id))
        }));
    }
    barrier.wait();
    let results = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let dispositions = store.list_dispositions(&scope).unwrap();
    assert_eq!(dispositions.len(), 1);
    assert_eq!(
        dispositions[0]
            .canonical_artifact
            .as_ref()
            .unwrap()
            .artifact_id
            .as_str(),
        "req_overtime"
    );

    let persisted: Vec<ProposalCard> = serde_json::from_str(&format!(
        "[{}]",
        std::fs::read_to_string(crate::shards::proposal_cards_path(&store.layout, &scope))
            .unwrap()
            .trim()
    ))
    .unwrap();
    assert_eq!(persisted[0].promotion_state, PromotionState::Proposed);
    assert_eq!(
        store.list_proposal_cards(&scope).unwrap()[0].promotion_state,
        PromotionState::Accepted
    );
    assert!(store.surface_proposals(&scope, &demand).unwrap().is_empty());
}

#[test]
fn validation_rejects_divergent_duplicate_before_landing_overlay() {
    let (_dir, store, scope) = initialized_store();
    let proposal = store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_duplicate",
            "Original",
            PromotionState::Proposed,
        ))
        .unwrap();
    let mut divergent = proposal;
    divergent.title = "Forged overlay".into();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::ideation_landings_path(&store.layout, &scope),
        &[crate::state_store::IdeationLandingBatch {
            contributions: Vec::new(),
            synthesis_packets: Vec::new(),
            proposals: vec![divergent],
            assertions: Vec::new(),
            dispositions: Vec::new(),
        }],
    )
    .unwrap();

    let error = store
        .validate_ideation_scope(&scope)
        .unwrap_err()
        .to_string();
    assert!(error.contains("duplicate immutable proposal"), "{error}");
}

#[test]
fn unregistered_terminal_row_is_not_legacy_compatibility() {
    let (_dir, store, scope) = initialized_store();
    let mut proposal = store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_forged",
            "Forged",
            PromotionState::Proposed,
        ))
        .unwrap();
    proposal.promotion_state = PromotionState::Accepted;
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::proposal_cards_path(&store.layout, &scope),
        &[proposal],
    )
    .unwrap();

    let error = store
        .validate_ideation_scope(&scope)
        .unwrap_err()
        .to_string();
    assert!(error.contains("frozen shipped-v1 fingerprint"), "{error}");
}

fn seed_proposal_ready_for_disposition(store: &StateStore, scope: &ScopeId) {
    allow_disposition_actor(store);
    let requirement_path = crate::shards::requirements_path(&store.layout, scope);
    std::fs::create_dir_all(requirement_path.parent().unwrap()).unwrap();
    std::fs::write(
        requirement_path,
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"req_overtime","statement":"Overtime","status":"active"}).to_string(),
    )
    .unwrap();
    let contribution: provenance_core::Contribution = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contribution_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_overtime", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_overtime", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_overtime"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })).unwrap();
    let mut synthesis: provenance_core::SynthesisPacket = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "synthesis_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [{"question": "Unverified", "needed_evidence_type": "source", "blocking_promotion": true}],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_overtime", "proposal_key": "overtime", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    })).unwrap();
    write_evidence(store, scope, &contribution, &synthesis);
    let mut proposal = proposal_input(
        scope,
        "proposal_overtime",
        "Overtime",
        PromotionState::Proposed,
    );
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    store.create_proposal_card(proposal).unwrap();
    synthesis.evidence_gaps.clear();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, scope),
        &[synthesis],
    )
    .unwrap();
    write_assertion(store, scope);
}

fn allow_disposition_actor(store: &StateStore) {
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("ben".into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

fn write_evidence(
    store: &StateStore,
    scope: &ScopeId,
    contribution: &provenance_core::Contribution,
    synthesis: &provenance_core::SynthesisPacket,
) {
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::contributions_path(&store.layout, scope),
        std::slice::from_ref(contribution),
    )
    .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, scope),
        std::slice::from_ref(synthesis),
    )
    .unwrap();
}

fn write_assertion(store: &StateStore, scope: &ScopeId) {
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::assertion_records_path(&store.layout, scope),
        &[provenance_core::AssertionRecord {
            schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
            scope_id: scope.clone(),
            id: provenance_core::AssertionId::new("assertion_overtime").unwrap(),
            proposal_id: StableId::new("proposal_overtime").unwrap(),
            synthesis_packet_id: StableId::new("synthesis_overtime").unwrap(),
            supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
        }],
    )
    .unwrap();
}

fn disposition_input(scope_id: ScopeId, id: &str) -> CreateDispositionInput {
    CreateDispositionInput {
        scope_id,
        id: StableId::new(id).unwrap(),
        proposal_id: StableId::new("proposal_overtime").unwrap(),
        decision: DispositionDecision::Accepted,
        rationale: "Reviewed".into(),
        actor: disposition_actor("ben"),
        canonical_artifact: Some(provenance_core::CanonicalArtifact {
            artifact_type: provenance_core::CanonicalArtifactType::Requirement,
            artifact_id: StableId::new("req_overtime").unwrap(),
        }),
        external_action: None,
    }
}

fn disposition_actor(id: &str) -> DispositionActor {
    DispositionActor {
        identity_type: IdentityType::Human,
        id: id.into(),
        name: None,
    }
}
