use crate::{
    layout::ProvenanceLayout,
    state_store::{
        CreateAssertionInput, CreateContributionInput, CreateDispositionInput,
        CreateProposalCardInput, CreateSynthesisPacketInput, StateStore,
    },
};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    AssertionId, DispositionActor, DispositionDecision, IdentityType, PromotionState,
    ProposalTraceability, ProposalType, ScopeId, StableId,
};

#[test]
fn modern_lifecycle_coexists_with_frozen_shipped_records() {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    let shipped = camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".provenance/state");
    copy_tree(&shipped, &layout.state_dir());
    let store = StateStore::new(layout);
    let scope = ScopeId::new("default").unwrap();
    land_modern_swarm_records(&store, &scope);
    store
        .create_proposal_card(CreateProposalCardInput {
            scope_id: scope.clone(),
            id: StableId::new("proposal_modern").unwrap(),
            proposal_key: "modern".into(),
            proposal_type: ProposalType::RequirementCandidate,
            title: "Modern".into(),
            summary: "Modern candidate".into(),
            confidence: None,
            traceability: ProposalTraceability {
                target: provenance_core::IdeationTarget {
                    artifact_type: provenance_core::IdeationTargetType::Requirement,
                    artifact_id: StableId::new("req_modern").unwrap(),
                },
                source_ids: vec![],
                evidence_references: vec![],
                supporting_claim_ids: vec![StableId::new("claim_modern").unwrap()],
            },
            promotion_state: PromotionState::Proposed,
            builds_on: vec![],
            duplicate_of: None,
            superseded_by: None,
        })
        .unwrap();
    close_the_evidence_gap(&store, &scope);
    store
        .assert_proposal(CreateAssertionInput {
            scope_id: scope.clone(),
            id: AssertionId::new("assertion_modern").unwrap(),
            proposal_id: StableId::new("proposal_modern").unwrap(),
            synthesis_packet_id: StableId::new("synthesis_modern").unwrap(),
            supporting_claim_ids: vec![StableId::new("claim_modern").unwrap()],
        })
        .unwrap();
    store
        .create_disposition(CreateDispositionInput {
            scope_id: scope,
            id: StableId::new("disposition_modern").unwrap(),
            proposal_id: StableId::new("proposal_modern").unwrap(),
            decision: DispositionDecision::Accepted,
            rationale: "Reviewed".into(),
            actor: DispositionActor {
                identity_type: IdentityType::Agent,
                id: "codex-review-panel-gpt55-medium".into(),
                name: None,
            },
            canonical_artifact: None,
            external_action: None,
        })
        .unwrap();
}

/// Lands the modern contribution and synthesis packet through the store's own
/// writers, so that they join the shipped records copied in above instead of
/// replacing the shards that hold them.
fn land_modern_swarm_records(store: &StateStore, scope: &ScopeId) {
    let shipped_contributions = store.list_contributions(scope).unwrap().len();
    let shipped_packets = store.list_synthesis_packets(scope).unwrap().len();
    let contribution: provenance_core::Contribution = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contribution_modern",
        "target": {"artifact_type": "requirement", "artifact_id": "req_modern"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_modern", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_modern", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_modern"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })).unwrap();
    let synthesis: provenance_core::SynthesisPacket = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "synthesis_modern",
        "target": {"artifact_type": "requirement", "artifact_id": "req_modern"}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [{"question": "Unverified", "needed_evidence_type": "source", "blocking_promotion": true}],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_modern", "proposal_key": "modern", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    })).unwrap();
    store
        .create_contribution(CreateContributionInput {
            scope_id: scope.clone(),
            id: contribution.id,
            target: contribution.target,
            participant_slot: contribution.participant_slot,
            stance: contribution.stance,
            strongest_finding: contribution.strongest_finding,
            evidence_references: contribution.evidence_references,
            material_claims: contribution.material_claims,
            risks: contribution.risks,
            objections: contribution.objections,
            challenges: contribution.challenges,
            suggested_artifact_changes: contribution.suggested_artifact_changes,
            unsupported_recommendations: contribution.unsupported_recommendations,
            uncertainty: contribution.uncertainty,
            open_questions: contribution.open_questions,
        })
        .unwrap();
    store
        .create_synthesis_packet(CreateSynthesisPacketInput {
            scope_id: scope.clone(),
            id: synthesis.id,
            target: synthesis.target,
            summary: synthesis.summary,
            consensus: synthesis.consensus,
            contested_claims: synthesis.contested_claims,
            minority_objections: synthesis.minority_objections,
            evidence_gaps: synthesis.evidence_gaps,
            unsupported_speculation: synthesis.unsupported_speculation,
            open_questions: synthesis.open_questions,
            suggested_artifacts: synthesis.suggested_artifacts,
            required_human_decisions: synthesis.required_human_decisions,
        })
        .unwrap();
    // The fixture joins the shipped records; it does not stand in for them.
    assert_eq!(
        store.list_contributions(scope).unwrap().len(),
        shipped_contributions + 1
    );
    assert_eq!(
        store.list_synthesis_packets(scope).unwrap().len(),
        shipped_packets + 1
    );
}

/// Closes the gap the packet opened, which is what lets the proposal be
/// asserted. A read-modify-write of the one record, so the shipped packets in
/// the same shard survive.
fn close_the_evidence_gap(store: &StateStore, scope: &ScopeId) {
    store
        .mutate_jsonl_records(
            &crate::shards::synthesis_packets_path(&store.layout, scope),
            |records: &mut Vec<provenance_core::SynthesisPacket>| {
                let packet = records
                    .iter_mut()
                    .find(|record| record.id.as_str() == "synthesis_modern")
                    .expect("the fixture packet was landed above");
                packet.evidence_gaps.clear();
                Ok(())
            },
        )
        .unwrap();
}

fn copy_tree(source: &camino::Utf8Path, destination: &camino::Utf8Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let child = camino::Utf8PathBuf::from_path_buf(entry.path()).unwrap();
        let target = destination.join(entry.file_name().to_string_lossy().as_ref());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&child, &target);
        } else {
            std::fs::copy(child, target).unwrap();
        }
    }
}
