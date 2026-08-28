use super::initialized_store;
use crate::state_store::{CreateProposalCardInput, ProposalDemand};
use provenance_core::{
    ArtifactLink, ArtifactLinkTargetType, IdeationEvidenceReference, IdeationEvidenceType,
    IdeationTarget, IdeationTargetType, PromotionState, ProposalTraceability, ProposalType,
    StableId, Topic, TopicStatus,
};
use provenance_macros::verifies;

mod properties;
mod topic_claims;

fn proposal_input(
    scope: &provenance_core::ScopeId,
    id: &str,
    target_type: IdeationTargetType,
    target_id: &str,
    path: Option<&str>,
    promotion_state: PromotionState,
) -> CreateProposalCardInput {
    CreateProposalCardInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        proposal_key: id.into(),
        proposal_type: ProposalType::RequirementCandidate,
        title: id.into(),
        summary: format!("Summary for {id}"),
        confidence: None,
        traceability: ProposalTraceability {
            target: IdeationTarget {
                artifact_type: target_type,
                artifact_id: StableId::new(target_id).unwrap(),
            },
            source_ids: Vec::new(),
            evidence_references: path
                .map(|path| IdeationEvidenceReference {
                    reference_id: StableId::new(format!("evidence_{id}")).unwrap(),
                    evidence_type: IdeationEvidenceType::Artifact,
                    summary: "Code evidence".into(),
                    file_path: Some(path.into()),
                    line: Some(42),
                })
                .into_iter()
                .collect(),
            supporting_claim_ids: Vec::new(),
        },
        builds_on: Vec::new(),
        promotion_state,
        duplicate_of: None,
        superseded_by: None,
    }
}

fn seed_asserted_proposal(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
) {
    let contribution: provenance_core::Contribution = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "contribution_a",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_a", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_a", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_a"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })).unwrap();
    let mut synthesis: provenance_core::SynthesisPacket = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "synthesis_a",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [{"question": "Unverified", "needed_evidence_type": "source", "blocking_promotion": true}],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_asserted", "proposal_key": "proposal_asserted", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    })).unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::contributions_path(&store.layout, scope),
        &[contribution],
    )
    .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, scope),
        &[synthesis.clone()],
    )
    .unwrap();
    let mut proposal = proposal_input(
        scope,
        "proposal_asserted",
        IdeationTargetType::Requirement,
        "req_overtime",
        Some("src/payroll.rs"),
        PromotionState::Proposed,
    );
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_a").unwrap()];
    store.create_proposal_card(proposal).unwrap();
    synthesis.evidence_gaps.clear();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, scope),
        &[synthesis],
    )
    .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::assertion_records_path(&store.layout, scope),
        &[provenance_core::AssertionRecord {
            schema_version: provenance_core::SchemaVersion(1),
            scope_id: scope.clone(),
            id: provenance_core::AssertionId::new("assertion_a").unwrap(),
            proposal_id: StableId::new("proposal_asserted").unwrap(),
            synthesis_packet_id: StableId::new("synthesis_a").unwrap(),
            supporting_claim_ids: vec![StableId::new("claim_a").unwrap()],
        }],
    )
    .unwrap();
}

#[test]
#[verifies("rule_proposal_surfacing", examples)]
fn changed_paths_surface_only_undisposed_proposals_with_matching_evidence_sites() {
    let (_dir, store, scope) = initialized_store();
    for input in [
        proposal_input(
            &scope,
            "proposal_matching",
            IdeationTargetType::Requirement,
            "req_overtime",
            Some("src/payroll.rs"),
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_other_path",
            IdeationTargetType::Requirement,
            "req_overtime",
            Some("src/leave.rs"),
            PromotionState::Proposed,
        ),
    ] {
        store.create_proposal_card(input).unwrap();
    }

    let surfaced = store
        .surface_proposals(
            &scope,
            &ProposalDemand::for_changed_paths(["src/payroll.rs"]),
        )
        .unwrap();

    assert_eq!(surfaced.len(), 1);
    assert_eq!(surfaced[0].proposal.id.as_str(), "proposal_matching");
    assert_eq!(
        serde_json::to_value(&surfaced[0].reasons).unwrap(),
        serde_json::json!([{"trigger":"evidence_site","path":"src/payroll.rs"}])
    );
}

#[test]
#[verifies("rule_proposal_surfacing", examples)]
fn evidence_paths_are_lexical_and_directory_aware_without_matching_siblings() {
    let (_dir, store, scope) = initialized_store();
    for input in [
        proposal_input(
            &scope,
            "proposal_file",
            IdeationTargetType::Requirement,
            "req_file",
            Some("./src/payroll.rs"),
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_directory",
            IdeationTargetType::Requirement,
            "req_directory",
            Some("src/payroll"),
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_sibling",
            IdeationTargetType::Requirement,
            "req_sibling",
            Some("src/pay"),
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_parent_escape",
            IdeationTargetType::Requirement,
            "req_parent_escape",
            Some("../../src/payroll.rs"),
            PromotionState::Proposed,
        ),
    ] {
        store.create_proposal_card(input).unwrap();
    }

    let surfaced = store
        .surface_proposals(
            &scope,
            &ProposalDemand::for_changed_paths([
                "src/payroll.rs",
                "./src/payroll/calculator.rs",
                "src/leave.rs",
            ]),
        )
        .unwrap();

    assert_eq!(
        surfaced
            .iter()
            .map(|surface| surface.proposal.id.as_str())
            .collect::<Vec<_>>(),
        vec!["proposal_directory", "proposal_file"]
    );
    assert_eq!(
        serde_json::to_value(&surfaced[0].reasons).unwrap(),
        serde_json::json!([{
            "trigger": "evidence_site",
            "path": "src/payroll/calculator.rs"
        }])
    );
    assert_eq!(
        serde_json::to_value(&surfaced[1].reasons).unwrap(),
        serde_json::json!([{"trigger": "evidence_site", "path": "src/payroll.rs"}])
    );
}

#[test]
#[verifies("rule_proposal_surfacing", examples)]
fn combined_demand_reports_deduplicated_reasons_in_deterministic_order() {
    let (_dir, store, scope) = initialized_store();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_matching",
            IdeationTargetType::Requirement,
            "req_overtime",
            Some("src/payroll.rs"),
            PromotionState::Proposed,
        ))
        .unwrap();
    let target = IdeationTarget {
        artifact_type: IdeationTargetType::Requirement,
        artifact_id: StableId::new("req_overtime").unwrap(),
    };

    let surfaced = store
        .surface_proposals(
            &scope,
            &ProposalDemand::new(
                vec!["src/payroll.rs".into(), "src/payroll.rs".into()],
                vec![target.clone(), target],
            ),
        )
        .unwrap();

    assert_eq!(
        surfaced[0].proposal.promotion_state,
        PromotionState::Proposed
    );
    assert_eq!(
        serde_json::to_value(&surfaced[0].reasons).unwrap(),
        serde_json::json!([
            {"trigger": "evidence_site", "path": "src/payroll.rs"},
            {
                "trigger": "territory",
                "target": {
                    "artifact_type": "requirement",
                    "artifact_id": "req_overtime"
                }
            }
        ])
    );
}

#[test]
#[verifies("rule_proposal_surfacing", examples)]
fn topic_claim_atomically_surfaces_matching_asserted_proposal_with_derived_state() {
    let (_dir, store, scope) = initialized_store();
    seed_asserted_proposal(&store, &scope);

    assert!(store
        .surface_proposals(&scope, &ProposalDemand::for_changed_paths(["src/other.rs"]),)
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .surface_proposals(
                &scope,
                &ProposalDemand::for_changed_paths(["src/payroll.rs"]),
            )
            .unwrap()
            .len(),
        1
    );

    let topic_id = StableId::new("topic_overtime").unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::topics_path(&store.layout, &scope),
        &[Topic {
            schema_version: provenance_core::SchemaVersion(1),
            scope_id: scope.clone(),
            id: topic_id.clone(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "Overtime".into(),
            status: TopicStatus::Open,
            claimed_by: None,
            claimed_at: None,
            links: Vec::new(),
        }],
    )
    .unwrap();

    let claim = store
        .claim_topic(&scope, &topic_id, "agent-one", Vec::<String>::new())
        .unwrap();

    assert_eq!(claim.topic.claimed_by.as_deref(), Some("agent-one"));
    assert_eq!(claim.surfaced_proposals.len(), 1);
    assert_eq!(
        claim.surfaced_proposals[0].proposal.promotion_state,
        PromotionState::Asserted
    );
    assert_eq!(
        serde_json::to_value(&claim.surfaced_proposals[0].reasons).unwrap(),
        serde_json::json!([{
            "trigger": "territory",
            "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"}
        }])
    );
}

#[test]
fn a_topic_claim_surfaces_proposals_in_its_explicit_territory() {
    let (_dir, store, scope) = initialized_store();
    let topic = Topic {
        schema_version: provenance_core::SchemaVersion(1),
        scope_id: scope.clone(),
        id: StableId::new("topic_overtime").unwrap(),
        requirement_id: StableId::new("req_overtime").unwrap(),
        title: "Overtime".into(),
        status: TopicStatus::Open,
        claimed_by: Some("agent-one".into()),
        claimed_at: Some(1),
        links: vec![ArtifactLink {
            target_type: ArtifactLinkTargetType::Rule,
            target_id: StableId::new("rule_overtime").unwrap(),
        }],
    };
    for input in [
        proposal_input(
            &scope,
            "proposal_topic",
            IdeationTargetType::Topic,
            "topic_overtime",
            None,
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_requirement",
            IdeationTargetType::Requirement,
            "req_overtime",
            None,
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_link",
            IdeationTargetType::Rule,
            "rule_overtime",
            None,
            PromotionState::Proposed,
        ),
        proposal_input(
            &scope,
            "proposal_outside",
            IdeationTargetType::Requirement,
            "req_leave",
            None,
            PromotionState::Proposed,
        ),
    ] {
        store.create_proposal_card(input).unwrap();
    }

    let surfaced = store
        .surface_proposals(
            &scope,
            &ProposalDemand::for_topic(&topic, Vec::<String>::new()),
        )
        .unwrap();

    assert_eq!(
        surfaced
            .iter()
            .map(|item| item.proposal.id.as_str())
            .collect::<Vec<_>>(),
        vec!["proposal_link", "proposal_requirement", "proposal_topic"]
    );
}

#[test]
fn proposal_demand_must_name_a_real_trigger() {
    let (_dir, store, scope) = initialized_store();

    let error = store
        .surface_proposals(
            &scope,
            &ProposalDemand::for_changed_paths(Vec::<String>::new()),
        )
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("at least one changed path or territory target"));
}
