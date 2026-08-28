#[test]
#[verifies("rule_proposal_authored_as_proposed", examples)]
fn modern_proposal_cannot_select_asserted_or_terminal_state() {
    for state in ["asserted", "accepted", "rejected", "deferred"] {
        let proposal: crate::ProposalCard = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "scope_id": "default",
            "id": format!("proposal_{state}"),
            "proposal_key": state,
            "proposal_type": "requirement_candidate",
            "title": "Candidate",
            "summary": "Candidate definition",
            "traceability": {
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                "source_ids": [],
                "evidence_references": [],
                "supporting_claim_ids": []
            },
            "promotion_state": state
        }))
        .unwrap();

        let error = crate::validate_proposal_intrinsic(&proposal)
            .unwrap_err()
            .to_string();
        assert!(error.contains("must begin proposed"), "{error}");
    }
}

#[test]
#[verifies("rule_proposal_authored_as_proposed", examples)]
fn modern_proposal_cannot_embed_disposition_authority() {
    let mut proposal = proposal_json("proposal_a", "assertion_parent");
    proposal.as_object_mut().unwrap().remove("builds_on");
    proposal["duplicate_of"] = serde_json::json!("proposal_parent");
    let proposal: crate::ProposalCard = serde_json::from_value(proposal).unwrap();

    let error = crate::validate_proposal_intrinsic(&proposal)
        .unwrap_err()
        .to_string();
    assert!(error.contains("disposition links"), "{error}");
}

#[test]
#[verifies("rule_proposal_lineage_acyclic", examples)]
fn lineage_rejects_missing_immutable_assertion_id() {
    let proposal: crate::ProposalCard = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "scope_id": "default",
        "id": "proposal_child",
        "proposal_key": "child",
        "proposal_type": "question",
        "title": "Child",
        "summary": "Child proposal",
        "traceability": {
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "source_ids": [],
            "evidence_references": [],
            "supporting_claim_ids": []
        },
        "promotion_state": "proposed",
        "builds_on": ["assertion_missing"]
    }))
    .unwrap();

    let error = crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&[]),
        contributions: &[],
        synthesis_packets: &[],
        proposals: &[proposal],
        assertions: &[],
        dispositions: &[],
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("assertion_missing does not exist"), "{error}");
}

#[test]
#[verifies("rule_proposal_lineage_acyclic", examples)]
fn lineage_rejects_cycles_through_assertion_owners() {
    let proposals: Vec<crate::ProposalCard> = serde_json::from_value(serde_json::json!([
        proposal_json("proposal_a", "assertion_b"),
        proposal_json("proposal_b", "assertion_a")
    ]))
    .unwrap();
    let assertions: Vec<crate::AssertionRecord> = serde_json::from_value(serde_json::json!([
        assertion_json("assertion_a", "proposal_a"),
        assertion_json("assertion_b", "proposal_b")
    ]))
    .unwrap();

    let error = crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&[]),
        contributions: &[],
        synthesis_packets: &[],
        proposals: &proposals,
        assertions: &assertions,
        dispositions: &[],
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("lineage contains a cycle"), "{error}");
}

fn proposal_json(id: &str, builds_on: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": id,
        "proposal_key": id, "proposal_type": "question", "title": id, "summary": id,
        "traceability": {
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
        },
        "promotion_state": "proposed", "builds_on": [builds_on]
    })
}

fn assertion_json(id: &str, proposal_id: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": id,
        "proposal_id": proposal_id, "synthesis_packet_id": "synthesis_a",
        "supporting_claim_ids": []
    })
}

#[test]
#[verifies("rule_positive_evidence", examples)]
fn assertion_rejects_unsupported_and_exploratory_evidence() {
    for evidence_type in ["unsupported", "exploratory"] {
        let (mut contribution, synthesis, proposal, assertion) = lifecycle_fixture();
        contribution["material_claims"][0]["evidence_type"] =
            serde_json::json!(evidence_type);
        contribution["evidence_references"][0]["evidence_type"] =
            serde_json::json!(evidence_type);
        let error = validate_fixture(contribution, synthesis, proposal, assertion)
            .unwrap_err()
            .to_string();
        assert!(error.contains("positive evidence type"), "{error}");
    }
}

#[test]
fn assertion_requires_positive_evidence_owned_by_proposal_target() {
    let (mut contribution, synthesis, proposal, assertion) = lifecycle_fixture();
    contribution["target"]["artifact_id"] = serde_json::json!("req_other");
    let error = validate_fixture(contribution, synthesis, proposal, assertion)
        .unwrap_err()
        .to_string();
    assert!(error.contains("owned by the proposal target"), "{error}");

    let (mut contribution, synthesis, proposal, assertion) = lifecycle_fixture();
    contribution["material_claims"][0]["evidence_reference_ids"] = serde_json::json!([]);
    let error = validate_fixture(contribution, synthesis, proposal, assertion)
        .unwrap_err()
        .to_string();
    assert!(error.contains("lacks positive evidence"), "{error}");
}

#[test]
fn assertion_rejects_evidence_reference_owned_by_multiple_contributions() {
    let (contribution, synthesis, proposal, assertion) = lifecycle_fixture();
    let mut duplicate_owner = contribution.clone();
    duplicate_owner["id"] = serde_json::json!("contribution_b");
    duplicate_owner["material_claims"] = serde_json::json!([]);
    let contributions = vec![
        serde_json::from_value(contribution).unwrap(),
        serde_json::from_value(duplicate_owner).unwrap(),
    ];
    let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
    let proposals = vec![serde_json::from_value(proposal).unwrap()];
    let assertions = vec![serde_json::from_value(assertion).unwrap()];

    let error = crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&[]),
        contributions: &contributions,
        synthesis_packets: &synthesis_packets,
        proposals: &proposals,
        assertions: &assertions,
        dispositions: &[],
    })
    .unwrap_err()
    .to_string();

    assert!(
        error.contains("evidence evidence_a must have exactly one owner"),
        "{error}"
    );
}

#[test]
#[verifies("rule_qualified_proposal_assertion", examples)]
fn assertion_requires_unblocked_uncontested_adjudication() {
    let (contribution, mut synthesis, proposal, assertion) = lifecycle_fixture();
    synthesis["evidence_gaps"] = serde_json::json!([{
        "question": "Unknown", "needed_evidence_type": "source", "blocking_promotion": true
    }]);
    let error = validate_fixture(contribution, synthesis, proposal, assertion)
        .unwrap_err()
        .to_string();
    assert!(error.contains("blocking evidence gap"), "{error}");

    let (contribution, mut synthesis, proposal, assertion) = lifecycle_fixture();
    synthesis["required_human_decisions"] = serde_json::json!([{
        "decision_key": "decision_needed", "prompt": "Choose", "blocks_promotion": true
    }]);
    let error = validate_fixture(contribution, synthesis, proposal, assertion)
        .unwrap_err()
        .to_string();
    assert!(error.contains("blocking human decision"), "{error}");

    let (contribution, mut synthesis, proposal, assertion) = lifecycle_fixture();
    synthesis["contested_claims"] = serde_json::json!([{
        "claim_id": "claim_a", "statement": "Disputed", "supporting_participant_slots": [],
        "opposing_participant_slots": ["refuter"], "evidence_quality": "weak"
    }]);
    let error = validate_fixture(contribution, synthesis, proposal, assertion)
        .unwrap_err()
        .to_string();
    assert!(error.contains("claim claim_a is contested"), "{error}");
}

#[test]
fn assertion_claim_matching_is_order_independent() {
    let (mut contribution, synthesis, mut proposal, mut assertion) = lifecycle_fixture();
    contribution["evidence_references"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "reference_id": "evidence_b", "evidence_type": "source", "summary": "Also pinned"
        }));
    contribution["material_claims"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "claim_id": "claim_b", "statement": "Also observed", "evidence_type": "source",
            "evidence_reference_ids": ["evidence_b"]
        }));
    proposal["traceability"]["supporting_claim_ids"] = serde_json::json!(["claim_a", "claim_b"]);
    assertion["supporting_claim_ids"] = serde_json::json!(["claim_b", "claim_a"]);

    validate_fixture(contribution, synthesis, proposal, assertion).unwrap();
}

#[test]
#[verifies("rule_proposal_effective_state", examples)]
fn effective_state_is_derived_from_immutable_assertion_and_disposition() {
    let (_, _, proposal, assertion) = lifecycle_fixture();
    let proposal: crate::ProposalCard = serde_json::from_value(proposal).unwrap();
    let assertion: crate::AssertionRecord = serde_json::from_value(assertion).unwrap();
    let disposition: crate::DispositionRecord = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "disposition_a",
        "proposal_id": "proposal_a", "decision": "accepted", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"}
    }))
    .unwrap();

    assert_eq!(
        crate::effective_proposal_state(&proposal, &[], &[]),
        crate::PromotionState::Proposed
    );
    assert_eq!(
        crate::effective_proposal_state(&proposal, std::slice::from_ref(&assertion), &[]),
        crate::PromotionState::Asserted
    );
    assert_eq!(
        crate::effective_proposal_state(&proposal, &[assertion], &[disposition]),
        crate::PromotionState::Accepted
    );
    assert_eq!(proposal.promotion_state, crate::PromotionState::Proposed);
}

#[test]
fn divergent_duplicate_immutable_ids_fail_closed() {
    let (_, _, proposal, _) = lifecycle_fixture();
    let first: crate::ProposalCard = serde_json::from_value(proposal.clone()).unwrap();
    let mut divergent = proposal;
    divergent["title"] = serde_json::json!("Forged replacement");
    let divergent: crate::ProposalCard = serde_json::from_value(divergent).unwrap();

    let error = crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&[]),
        contributions: &[],
        synthesis_packets: &[],
        proposals: &[first, divergent],
        assertions: &[],
        dispositions: &[],
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("duplicate immutable proposal"), "{error}");
}

#[test]
fn byte_identical_duplicate_immutable_ids_fail_closed() {
    let (_, _, proposal, _) = lifecycle_fixture();
    let proposal: crate::ProposalCard = serde_json::from_value(proposal).unwrap();

    let error = crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&[]),
        contributions: &[],
        synthesis_packets: &[],
        proposals: &[proposal.clone(), proposal],
        assertions: &[],
        dispositions: &[],
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("duplicate immutable proposal"), "{error}");
}

#[test]
fn modern_policy_rejects_terminal_proposal_rows() {
    let (contribution, synthesis, mut proposal, assertion) = lifecycle_fixture();
    proposal["promotion_state"] = serde_json::json!("accepted");
    let error = validate_fixture(contribution, synthesis, proposal, assertion)
        .unwrap_err()
        .to_string();
    assert!(error.contains("modern-only lifecycle policy"), "{error}");
}

#[test]
#[verifies("rule_proposal_effective_state", examples)]
fn frozen_terminal_effective_state_ignores_lifecycle_records() {
    let (_, _, mut proposal, assertion) = lifecycle_fixture();
    proposal["promotion_state"] = serde_json::json!("rejected");
    let proposal: crate::ProposalCard = serde_json::from_value(proposal).unwrap();
    let assertion: crate::AssertionRecord = serde_json::from_value(assertion).unwrap();
    let disposition: crate::DispositionRecord = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "disposition_forged",
        "proposal_id": "proposal_a", "decision": "accepted", "rationale": "Forged",
        "actor": {"identity_type": "human", "id": "reviewer"}
    })).unwrap();
    assert_eq!(
        crate::effective_proposal_state(&proposal, &[assertion], &[disposition]),
        crate::PromotionState::Rejected
    );
}

fn validate_fixture(
    contribution: serde_json::Value,
    synthesis: serde_json::Value,
    proposal: serde_json::Value,
    assertion: serde_json::Value,
) -> anyhow::Result<()> {
    let contributions = vec![serde_json::from_value(contribution).unwrap()];
    let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
    let proposals = vec![serde_json::from_value(proposal).unwrap()];
    let assertions = vec![serde_json::from_value(assertion).unwrap()];
    crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&[]),
        contributions: &contributions,
        synthesis_packets: &synthesis_packets,
        proposals: &proposals,
        assertions: &assertions,
        dispositions: &[],
    })
}

fn lifecycle_fixture() -> (
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
) {
    let contribution = serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "contribution_a",
        "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
        "participant_slot": "extractor", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_a", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_a", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_a"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    });
    let synthesis = serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "synthesis_a",
        "target": {"artifact_type": "requirement", "artifact_id": "req_a"}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [], "evidence_gaps": [],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_a", "proposal_key": "proposal-a", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["extractor"]}],
        "required_human_decisions": []
    });
    let proposal = serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "proposal_a", "proposal_key": "proposal-a",
        "proposal_type": "requirement_candidate", "title": "Candidate", "summary": "Candidate",
        "traceability": {"target": {"artifact_type": "requirement", "artifact_id": "req_a"}, "source_ids": [], "evidence_references": [], "supporting_claim_ids": ["claim_a"]},
        "promotion_state": "proposed"
    });
    let assertion = serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "assertion_a", "proposal_id": "proposal_a",
        "synthesis_packet_id": "synthesis_a", "supporting_claim_ids": ["claim_a"]
    });
    (contribution, synthesis, proposal, assertion)
}
