//! The rbac window matrix at the store seam: batch landings run the
//! family-12 ratification per row, and an rbac-only repository cannot
//! deadlock a valid disposition the way the legacy empty allowlist did.

use super::initialized_store;
use crate::state_store::{
    CreateContributionInput, CreateDispositionInput, CreateProposalCardInput,
    CreateSynthesisPacketInput, IdeationLandingBatch,
};
use provenance_core::{
    DispositionActor, DispositionDecision, IdeationTarget, IdeationTargetType, IdentityType,
    PromotionState, ProposalTraceability, ProposalType, RbacClaim, ScopeId, StableId,
};
#[allow(clippy::unnecessary_wraps)] // models the caller's optional claim
fn claim(actor: &str) -> Option<RbacClaim> {
    Some(RbacClaim::new(actor).unwrap())
}

/// Installs an rbac-only manifest with the named assignments.
fn install_rbac_manifest(
    store: &crate::state_store::StateStore,
    assignments: &[(&str, Option<IdentityType>, &[&str])],
) {
    let mut manifest = store.manifest().unwrap();
    manifest.rbac = Some(provenance_core::RbacSection {
        assignments: assignments
            .iter()
            .map(
                |(actor_id, identity_type, capabilities)| provenance_core::Assignment {
                    actor_id: actor_id.to_string(),
                    identity_type: *identity_type,
                    capabilities: capabilities
                        .iter()
                        .map(|capability| provenance_core::Capability::parse(capability).unwrap())
                        .collect(),
                    scopes: vec!["default".to_string()],
                },
            )
            .collect(),
    });
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

fn actor(id: &str, identity_type: IdentityType) -> DispositionActor {
    DispositionActor {
        identity_type,
        id: id.into(),
        name: None,
    }
}

fn disposition(id: &str, actor: DispositionActor) -> provenance_core::DispositionRecord {
    provenance_core::DispositionRecord {
        schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        proposal_id: StableId::new("proposal_overtime").unwrap(),
        decision: DispositionDecision::Rejected,
        rationale: "Reviewed".into(),
        actor,
        canonical_artifact: None,
        external_action: None,
    }
}

/// Seeds a live (proposed) proposal directly through the store before any
/// manifest regime is installed. The synthesis carries a blocking evidence
/// gap, so the proposal is live but unqualified and needs no assertion.
fn seed_asserted_proposal(store: &crate::state_store::StateStore, scope: &ScopeId) {
    store
        .create_proposal_card(
            None,
            CreateProposalCardInput {
                scope_id: scope.clone(),
                id: StableId::new("proposal_overtime").unwrap(),
                proposal_key: "overtime".into(),
                proposal_type: ProposalType::RequirementCandidate,
                title: "Overtime".into(),
                summary: "Clarify the overtime requirement.".into(),
                confidence: None,
                traceability: ProposalTraceability {
                    target: IdeationTarget {
                        artifact_type: IdeationTargetType::Requirement,
                        artifact_id: StableId::new("req_overtime").unwrap(),
                    },
                    source_ids: Vec::new(),
                    evidence_references: Vec::new(),
                    supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
                },
                builds_on: Vec::new(),
                promotion_state: PromotionState::Proposed,
                duplicate_of: None,
                superseded_by: None,
            },
        )
        .unwrap();
    store
        .create_synthesis_packet(
            None,
            CreateSynthesisPacketInput {
                scope_id: scope.clone(),
                id: StableId::new("synthesis_overtime").unwrap(),
                target: IdeationTarget {
                    artifact_type: IdeationTargetType::Requirement,
                    artifact_id: StableId::new("req_overtime").unwrap(),
                },
                summary: "Adjudicated".into(),
                consensus: Vec::new(),
                contested_claims: Vec::new(),
                minority_objections: Vec::new(),
                evidence_gaps: vec![provenance_core::EvidenceGap {
                    question: "Unverified".into(),
                    needed_evidence_type: provenance_core::IdeationEvidenceType::Source,
                    blocking_promotion: true,
                }],
                unsupported_speculation: Vec::new(),
                open_questions: Vec::new(),
                suggested_artifacts: Vec::new(),
                required_human_decisions: Vec::new(),
            },
        )
        .unwrap();
    store
        .create_contribution(
            None,
            CreateContributionInput {
                scope_id: scope.clone(),
                id: StableId::new("contribution_overtime").unwrap(),
                target: IdeationTarget {
                    artifact_type: IdeationTargetType::Requirement,
                    artifact_id: StableId::new("req_overtime").unwrap(),
                },
                participant_slot: "reviewer".into(),
                stance: provenance_core::ContributionStance::Support,
                strongest_finding: "Observed".into(),
                evidence_references: Vec::new(),
                material_claims: Vec::new(),
                risks: Vec::new(),
                objections: Vec::new(),
                challenges: Vec::new(),
                suggested_artifact_changes: Vec::new(),
                unsupported_recommendations: Vec::new(),
                uncertainty: provenance_core::UncertaintyRating {
                    level: provenance_core::UncertaintyLevel::Low,
                    rationale: "Direct".into(),
                },
                open_questions: Vec::new(),
            },
        )
        .unwrap();
}

/// A live proposal keeps the shipped promotion state vocabulary.
const _PROMOTION_STATE: PromotionState = PromotionState::Proposed;

#[test]
fn a_batch_mixing_contributions_and_dispositions_passes_or_fails_per_row() {
    let (_dir, store, scope) = initialized_store();
    seed_asserted_proposal(&store, &scope);
    install_rbac_manifest(
        &store,
        &[
            ("reviewer", Some(IdentityType::Human), &["edit", "execute"]),
            ("robot", Some(IdentityType::Agent), &["execute"]),
        ],
    );

    let batch = IdeationLandingBatch {
        dispositions: vec![disposition(
            "disposition_robot",
            actor("robot", IdentityType::Agent),
        )],
        contributions: Vec::new(),
        synthesis_packets: Vec::new(),
        proposals: Vec::new(),
        assertions: Vec::new(),
    };
    let error = store
        .land_ideation_batch(claim("reviewer").as_ref(), &scope, batch, false)
        .unwrap_err();
    assert!(
        error.to_string().contains(
            "rbac: disposition actor robot needs an assignment with identity_type human to end a live proposal",
        ),
        "{error}"
    );

    let batch = IdeationLandingBatch {
        dispositions: vec![disposition(
            "disposition_human",
            actor("reviewer", IdentityType::Human),
        )],
        contributions: Vec::new(),
        synthesis_packets: Vec::new(),
        proposals: Vec::new(),
        assertions: Vec::new(),
    };
    store
        .land_ideation_batch(claim("reviewer").as_ref(), &scope, batch, false)
        .unwrap();
}

#[test]
fn an_rbac_only_repository_cannot_deadlock_a_valid_disposition() {
    let (_dir, store, scope) = initialized_store();
    seed_asserted_proposal(&store, &scope);
    // The manifest carries the always-shipped empty legacy list beside the
    // section — unambiguous — and the legacy empty-list law would have
    // blocked every disposition.
    install_rbac_manifest(
        &store,
        &[("reviewer", Some(IdentityType::Human), &["edit", "execute"])],
    );

    store
        .create_disposition(
            claim("reviewer").as_ref(),
            CreateDispositionInput {
                scope_id: scope,
                id: StableId::new("disposition_overtime").unwrap(),
                proposal_id: StableId::new("proposal_overtime").unwrap(),
                decision: DispositionDecision::Rejected,
                rationale: "Reviewed".into(),
                actor: actor("reviewer", IdentityType::Human),
                canonical_artifact: None,
                external_action: None,
            },
        )
        .expect("a human assignment admits the disposition the old law would block");
}
