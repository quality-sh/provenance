use super::initialized_store;
use crate::state_store::{
    CreateContributionInput, CreateSynthesisPacketInput, IdeationLandingBatch,
};
use provenance_core::{
    Contribution, ContributionStance, IdeationTarget, IdeationTargetType, SchemaVersion, StableId,
    SynthesisPacket, UncertaintyLevel, UncertaintyRating,
};
use provenance_macros::verifies;

#[test]
fn ideation_output_records_are_written_deterministically() {
    let (_dir, store, scope) = initialized_store();

    store
        .create_contribution(
            None,
            CreateContributionInput {
                scope_id: scope.clone(),
                id: StableId::new("contrib_b").unwrap(),
                target: IdeationTarget {
                    artifact_type: IdeationTargetType::Requirement,
                    artifact_id: StableId::new("req_overtime").unwrap(),
                },
                participant_slot: "reviewer".into(),
                stance: ContributionStance::Support,
                strongest_finding: "Supported by evidence".into(),
                evidence_references: Vec::new(),
                material_claims: Vec::new(),
                risks: Vec::new(),
                objections: Vec::new(),
                challenges: Vec::new(),
                suggested_artifact_changes: Vec::new(),
                unsupported_recommendations: Vec::new(),
                uncertainty: UncertaintyRating {
                    level: UncertaintyLevel::Low,
                    rationale: "Direct evidence".into(),
                },
                open_questions: Vec::new(),
            },
        )
        .unwrap();
    store
        .create_contribution(
            None,
            CreateContributionInput {
                scope_id: scope.clone(),
                id: StableId::new("contrib_a").unwrap(),
                target: IdeationTarget {
                    artifact_type: IdeationTargetType::Requirement,
                    artifact_id: StableId::new("req_overtime").unwrap(),
                },
                participant_slot: "refuter".into(),
                stance: ContributionStance::NeedsMoreEvidence,
                strongest_finding: "Needs more evidence".into(),
                evidence_references: Vec::new(),
                material_claims: Vec::new(),
                risks: Vec::new(),
                objections: Vec::new(),
                challenges: Vec::new(),
                suggested_artifact_changes: Vec::new(),
                unsupported_recommendations: Vec::new(),
                uncertainty: UncertaintyRating {
                    level: UncertaintyLevel::High,
                    rationale: "Missing source".into(),
                },
                open_questions: Vec::new(),
            },
        )
        .unwrap();

    assert_eq!(
        store.list_contributions(&scope).unwrap()[0].id.as_str(),
        "contrib_a"
    );
}

#[test]
fn invalid_lifecycle_batch_is_rejected_without_partial_writes() {
    let (_dir, store, scope) = initialized_store();
    let batch: crate::state_store::IdeationLandingBatch =
        serde_json::from_value(serde_json::json!({
            "contributions": [{
                "schema_version": 1, "scope_id": "default", "id": "contribution_a",
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                "participant_slot": "extractor", "stance": "support", "strongest_finding": "Observed",
                "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
                "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
                "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
            }],
            "synthesis_packets": [],
            "proposals": [],
            "assertions": [{
                "schema_version": 1, "scope_id": "default", "id": "assertion_bad",
                "proposal_id": "proposal_missing", "synthesis_packet_id": "synthesis_missing",
                "supporting_claim_ids": []
            }],
            "dispositions": []
        }))
        .unwrap();

    store
        .land_ideation_batch(None, &scope, batch, false)
        .unwrap_err();
    assert!(store.list_contributions(&scope).unwrap().is_empty());
    assert!(store.list_assertion_records(&scope).unwrap().is_empty());
}

#[test]
fn direct_contribution_create_and_replace_respect_landed_records() {
    let (_dir, store, scope) = initialized_store();
    store
        .land_ideation_batch(
            None,
            &scope,
            IdeationLandingBatch {
                contributions: vec![contribution(&scope, "landed")],
                synthesis_packets: Vec::new(),
                proposals: Vec::new(),
                assertions: Vec::new(),
                dispositions: Vec::new(),
            },
            false,
        )
        .unwrap();

    let error = store
        .create_contribution(None, contribution_input(&scope, "direct"))
        .unwrap_err();
    assert!(error.to_string().contains("contribution already exists"));

    store
        .upsert_contribution(None, contribution_input(&scope, "replacement"))
        .unwrap();
    let records = store.list_contributions(&scope).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].strongest_finding, "replacement");
}

#[test]
fn direct_synthesis_create_and_replace_respect_landed_records() {
    let (_dir, store, scope) = initialized_store();
    store
        .land_ideation_batch(
            None,
            &scope,
            IdeationLandingBatch {
                contributions: Vec::new(),
                synthesis_packets: vec![synthesis_packet(&scope, "landed")],
                proposals: Vec::new(),
                assertions: Vec::new(),
                dispositions: Vec::new(),
            },
            false,
        )
        .unwrap();

    let error = store
        .create_synthesis_packet(None, synthesis_input(&scope, "direct"))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("synthesis packet already exists"));

    store
        .upsert_synthesis_packet(None, synthesis_input(&scope, "replacement"))
        .unwrap();
    let records = store.list_synthesis_packets(&scope).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].summary, "replacement");
}

#[test]
fn composite_ideation_list_holds_publication_lock_between_reads() {
    let (_dir, store, scope) = initialized_store();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::contributions_path(&store.layout, &scope),
        &[contribution(&scope, "direct")],
    )
    .unwrap();
    let (direct_read_tx, direct_read_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let reader = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store.list_contributions_after_direct_read(&scope, || {
                direct_read_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
        })
    };
    direct_read_rx.recv().unwrap();

    let (published_tx, published_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|| {
                    crate::jsonl::write_jsonl_atomic::<Contribution>(
                        &crate::shards::contributions_path(&store.layout, &scope),
                        &[],
                    )?;
                    crate::jsonl::write_jsonl_atomic(
                        &crate::shards::ideation_landings_path(&store.layout, &scope),
                        &[IdeationLandingBatch {
                            contributions: vec![contribution(&scope, "landed")],
                            synthesis_packets: Vec::new(),
                            proposals: Vec::new(),
                            assertions: Vec::new(),
                            dispositions: Vec::new(),
                        }],
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
    let records = reader.join().unwrap().unwrap();
    publisher.join().unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].strongest_finding, "direct");
    assert_eq!(
        store.list_contributions(&scope).unwrap()[0].strongest_finding,
        "landed"
    );
}

#[test]
#[verifies("rule_asserted_evidence_immutable", examples)]
fn direct_replacement_cannot_retarget_asserted_evidence() {
    let (_dir, store, scope) = initialized_store();
    let batch: IdeationLandingBatch = serde_json::from_value(serde_json::json!({
        "contributions": [{
            "schema_version": 1, "scope_id": "default", "id": "contribution_landed",
            "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
            "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
            "evidence_references": [{"reference_id": "evidence_a", "evidence_type": "source", "summary": "Pinned"}],
            "material_claims": [{"claim_id": "claim_a", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_a"]}],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
        }],
        "synthesis_packets": [{
            "schema_version": 1, "scope_id": "default", "id": "synthesis_landed",
            "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"}, "summary": "Adjudicated",
            "consensus": [], "contested_claims": [], "minority_objections": [], "evidence_gaps": [],
            "unsupported_speculation": [], "open_questions": [],
            "suggested_artifacts": [{"proposal_id": "proposal_a", "proposal_key": "proposal-a", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
            "required_human_decisions": []
        }],
        "proposals": [{
            "schema_version": 1, "scope_id": "default", "id": "proposal_a", "proposal_key": "proposal-a",
            "proposal_type": "requirement_candidate", "title": "Candidate", "summary": "Candidate",
            "traceability": {"target": {"artifact_type": "requirement", "artifact_id": "req_overtime"}, "source_ids": [], "evidence_references": [], "supporting_claim_ids": ["claim_a"]},
            "promotion_state": "proposed"
        }],
        "assertions": [{
            "schema_version": 1, "scope_id": "default", "id": "assertion_a", "proposal_id": "proposal_a",
            "synthesis_packet_id": "synthesis_landed", "supporting_claim_ids": ["claim_a"]
        }],
        "dispositions": []
    }))
    .unwrap();
    store
        .land_ideation_batch(None, &scope, batch, false)
        .unwrap();

    let contribution_error = store
        .upsert_contribution(None, contribution_input(&scope, "replacement"))
        .unwrap_err()
        .to_string();
    assert!(contribution_error.contains("referenced by an assertion"));
    let synthesis_error = store
        .upsert_synthesis_packet(None, synthesis_input(&scope, "replacement"))
        .unwrap_err()
        .to_string();
    assert!(synthesis_error.contains("referenced by an assertion"));
    assert_eq!(
        store.list_contributions(&scope).unwrap()[0].strongest_finding,
        "Observed"
    );
    assert_eq!(
        store.list_synthesis_packets(&scope).unwrap()[0].summary,
        "Adjudicated"
    );
}

fn contribution_input(scope: &provenance_core::ScopeId, finding: &str) -> CreateContributionInput {
    CreateContributionInput {
        scope_id: scope.clone(),
        id: StableId::new("contribution_landed").unwrap(),
        target: target(),
        participant_slot: "reviewer".into(),
        stance: ContributionStance::Support,
        strongest_finding: finding.into(),
        evidence_references: Vec::new(),
        material_claims: Vec::new(),
        risks: Vec::new(),
        objections: Vec::new(),
        challenges: Vec::new(),
        suggested_artifact_changes: Vec::new(),
        unsupported_recommendations: Vec::new(),
        uncertainty: UncertaintyRating {
            level: UncertaintyLevel::Low,
            rationale: "Direct".into(),
        },
        open_questions: Vec::new(),
    }
}

fn contribution(scope: &provenance_core::ScopeId, finding: &str) -> Contribution {
    let input = contribution_input(scope, finding);
    Contribution {
        schema_version: SchemaVersion(1),
        scope_id: input.scope_id,
        id: input.id,
        target: input.target,
        participant_slot: input.participant_slot,
        stance: input.stance,
        strongest_finding: input.strongest_finding,
        evidence_references: input.evidence_references,
        material_claims: input.material_claims,
        risks: input.risks,
        objections: input.objections,
        challenges: input.challenges,
        suggested_artifact_changes: input.suggested_artifact_changes,
        unsupported_recommendations: input.unsupported_recommendations,
        uncertainty: input.uncertainty,
        open_questions: input.open_questions,
    }
}

fn synthesis_input(scope: &provenance_core::ScopeId, summary: &str) -> CreateSynthesisPacketInput {
    CreateSynthesisPacketInput {
        scope_id: scope.clone(),
        id: StableId::new("synthesis_landed").unwrap(),
        target: target(),
        summary: summary.into(),
        consensus: Vec::new(),
        contested_claims: Vec::new(),
        minority_objections: Vec::new(),
        evidence_gaps: Vec::new(),
        unsupported_speculation: Vec::new(),
        open_questions: Vec::new(),
        suggested_artifacts: Vec::new(),
        required_human_decisions: Vec::new(),
    }
}

fn synthesis_packet(scope: &provenance_core::ScopeId, summary: &str) -> SynthesisPacket {
    let input = synthesis_input(scope, summary);
    SynthesisPacket {
        schema_version: SchemaVersion(1),
        scope_id: input.scope_id,
        id: input.id,
        target: input.target,
        summary: input.summary,
        consensus: input.consensus,
        contested_claims: input.contested_claims,
        minority_objections: input.minority_objections,
        evidence_gaps: input.evidence_gaps,
        unsupported_speculation: input.unsupported_speculation,
        open_questions: input.open_questions,
        suggested_artifacts: input.suggested_artifacts,
        required_human_decisions: input.required_human_decisions,
    }
}

fn target() -> IdeationTarget {
    IdeationTarget {
        artifact_type: IdeationTargetType::Requirement,
        artifact_id: StableId::new("req_overtime").unwrap(),
    }
}
