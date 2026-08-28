//! Fixtures shared by the lifecycle validation tests.

use crate::state_store::CreateDispositionInput;
use provenance_core::{DispositionActor, DispositionDecision, IdentityType, StableId};

pub(super) fn seed_blocked_evidence(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
) {
    let contribution: provenance_core::Contribution = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "contribution_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_overtime", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_overtime", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_overtime"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })).unwrap();
    let synthesis: provenance_core::SynthesisPacket = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "synthesis_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [{"question": "Unverified", "needed_evidence_type": "source", "blocking_promotion": true}],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_overtime", "proposal_key": "overtime", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    })).unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::contributions_path(&store.layout, scope),
        &[contribution],
    )
    .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, scope),
        &[synthesis],
    )
    .unwrap();
}

pub(super) fn allow_actor(store: &crate::state_store::StateStore, id: &str) {
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push(id.into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

pub(super) fn disposition_input(
    scope_id: provenance_core::ScopeId,
    actor_id: &str,
) -> CreateDispositionInput {
    CreateDispositionInput {
        scope_id,
        id: StableId::new("disposition_overtime").unwrap(),
        proposal_id: StableId::new("proposal_overtime").unwrap(),
        decision: DispositionDecision::Accepted,
        rationale: "Reviewed".into(),
        actor: actor(actor_id),
        canonical_artifact: None,
        external_action: None,
    }
}

pub(super) fn actor(id: &str) -> DispositionActor {
    DispositionActor {
        identity_type: IdentityType::Human,
        id: id.into(),
        name: None,
    }
}
