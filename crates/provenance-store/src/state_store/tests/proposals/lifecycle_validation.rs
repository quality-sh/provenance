use super::{super::initialized_store, proposal_input};
use crate::state_store::{CreateAssertionInput, CreateDispositionInput, ProposalDemand};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    DispositionActor, DispositionDecision, IdeationTarget, IdeationTargetType, IdentityType,
    PromotionState, StableId,
};
use provenance_macros::verifies;

#[test]
fn legacy_disposition_path_reads_shipped_camel_case_records() {
    let (_dir, store, scope) = initialized_store();
    let path = crate::shards::legacy_promotion_decisions_path(&store.layout, &scope);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","promotionDecisionId":"disposition_legacy","proposalId":"proposal_legacy","decision":"accepted","rationale":"Accepted.","decidedBy":{"identityType":"human","userId":"reviewer"},"canonicalArtifact":{"artifactType":"requirement","artifactId":"requirement_legacy"}}).to_string(),
    )
    .unwrap();

    let dispositions = store.list_dispositions(&scope).unwrap();
    assert_eq!(dispositions[0].id.as_str(), "disposition_legacy");
    assert_eq!(dispositions[0].actor.id, "reviewer");
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn deprecated_disposition_shard_rejects_modern_records() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store, "reviewer");
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_modern",
            "Modern proposal",
            PromotionState::Proposed,
        ))
        .unwrap();
    let path = crate::shards::legacy_promotion_decisions_path(&store.layout, &scope);
    std::fs::write(
        path,
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","promotionDecisionId":"disposition_modern","proposalId":"proposal_modern","decision":"rejected","rationale":"Rejected.","decidedBy":{"identityType":"human","userId":"reviewer"},"canonicalArtifact":null}).to_string(),
    )
    .unwrap();

    let error = store
        .validate_ideation_scope(&scope)
        .unwrap_err()
        .to_string();

    // The full message, so the assertion cannot be satisfied by the separate
    // shipped-v1 fingerprint errors that share the phrase "frozen shipped-v1".
    assert!(
        error.contains(
            "deprecated promotion_decisions.jsonl accepts only the frozen shipped-v1 \
             disposition audit"
        ),
        "{error}"
    );
}

#[test]
fn direct_modern_proposal_write_rejects_terminal_ingress() {
    let (_dir, store, scope) = initialized_store();
    let error = store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_forged",
            "Forged terminal",
            PromotionState::Accepted,
        ))
        .unwrap_err()
        .to_string();

    assert!(error.contains("must begin proposed"), "{error}");
    assert!(store.list_proposal_cards(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn accepted_disposition_requires_an_assertion() {
    let (_dir, store, scope) = initialized_store();
    seed_blocked_evidence(&store, &scope);
    let mut proposal = proposal_input(
        &scope,
        "proposal_overtime",
        "Overtime",
        PromotionState::Proposed,
    );
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    store.create_proposal_card(proposal).unwrap();

    let error = store
        .create_disposition(disposition_input(scope, "ben"))
        .unwrap_err()
        .to_string();

    assert!(error.contains("must be asserted"), "{error}");
}

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn rejected_disposition_does_not_require_an_assertion() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store, "ben");
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
            actor: actor("ben"),
            canonical_artifact: None,
            external_action: None,
        })
        .unwrap();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_deferred",
            "Deferred",
            PromotionState::Proposed,
        ))
        .unwrap();
    store
        .create_disposition(CreateDispositionInput {
            scope_id: scope.clone(),
            id: StableId::new("disposition_deferred").unwrap(),
            proposal_id: StableId::new("proposal_deferred").unwrap(),
            decision: DispositionDecision::Deferred,
            rationale: "Wait for policy".into(),
            actor: actor("ben"),
            canonical_artifact: None,
            external_action: None,
        })
        .unwrap();

    assert_eq!(
        store
            .list_proposal_cards(&scope)
            .unwrap()
            .into_iter()
            .map(|proposal| proposal.promotion_state)
            .collect::<Vec<_>>(),
        vec![PromotionState::Deferred, PromotionState::Rejected]
    );
    assert!(store
        .surface_proposals(
            &scope,
            &ProposalDemand::for_target(IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: StableId::new("req_overtime").unwrap(),
            }),
        )
        .unwrap()
        .is_empty());
}

#[test]
fn direct_assertion_uses_the_aggregate_evidence_validator() {
    let (_dir, store, scope) = initialized_store();
    seed_blocked_evidence(&store, &scope);
    let mut proposal = proposal_input(
        &scope,
        "proposal_overtime",
        "Overtime",
        PromotionState::Proposed,
    );
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    store.create_proposal_card(proposal).unwrap();

    let error = store
        .assert_proposal(CreateAssertionInput {
            scope_id: scope,
            id: provenance_core::AssertionId::new("assertion_overtime").unwrap(),
            proposal_id: StableId::new("proposal_overtime").unwrap(),
            synthesis_packet_id: StableId::new("synthesis_missing").unwrap(),
            supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
        })
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("synthesis packet") || error.contains("positive evidence"),
        "{error}"
    );
}

#[test]
fn assertion_rejects_conflicting_duplicate_evidence_ids() {
    let (_dir, store, scope) = initialized_store();
    seed_blocked_evidence(&store, &scope);
    let mut contributions = store.list_contributions(&scope).unwrap();
    contributions[0]
        .evidence_references
        .push(provenance_core::IdeationEvidenceReference {
            reference_id: StableId::new("evidence_overtime").unwrap(),
            evidence_type: provenance_core::IdeationEvidenceType::Unsupported,
            summary: "Conflicting unsupported evidence".into(),
            file_path: None,
            line: None,
        });
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::contributions_path(&store.layout, &scope),
        &contributions,
    )
    .unwrap();
    let mut proposal = proposal_input(
        &scope,
        "proposal_overtime",
        "Overtime",
        PromotionState::Proposed,
    );
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    store.create_proposal_card(proposal).unwrap();
    let mut packets = store.list_synthesis_packets(&scope).unwrap();
    packets[0].evidence_gaps.clear();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, &scope),
        &packets,
    )
    .unwrap();

    let error = store
        .assert_proposal(CreateAssertionInput {
            scope_id: scope,
            id: provenance_core::AssertionId::new("assertion_overtime").unwrap(),
            proposal_id: StableId::new("proposal_overtime").unwrap(),
            synthesis_packet_id: StableId::new("synthesis_overtime").unwrap(),
            supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
        })
        .unwrap_err()
        .to_string();

    assert!(error.contains("exactly one owner"), "{error}");
}

#[test]
fn resolving_one_human_decision_preserves_other_blockers() {
    let (_dir, store, scope) = initialized_store();
    seed_blocked_evidence(&store, &scope);
    let mut packets = store.list_synthesis_packets(&scope).unwrap();
    packets[0].evidence_gaps.clear();
    packets[0].required_human_decisions = serde_json::from_value(serde_json::json!([
        {"decision_key": "decision_a", "prompt": "Choose A", "blocks_promotion": true},
        {"decision_key": "decision_b", "prompt": "Choose B", "blocks_promotion": true}
    ]))
    .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, &scope),
        &packets,
    )
    .unwrap();
    let mut proposal = proposal_input(
        &scope,
        "proposal_overtime",
        "Overtime",
        PromotionState::Proposed,
    );
    proposal.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    store.create_proposal_card(proposal).unwrap();

    let error = store
        .assert_proposal_after_human_decision(
            CreateAssertionInput {
                scope_id: scope.clone(),
                id: provenance_core::AssertionId::new("assertion_overtime").unwrap(),
                proposal_id: StableId::new("proposal_overtime").unwrap(),
                synthesis_packet_id: StableId::new("synthesis_overtime").unwrap(),
                supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
            },
            &[StableId::new("decision_a").unwrap()],
        )
        .unwrap_err()
        .to_string();

    assert!(error.contains("blocking human decision"), "{error}");
    assert_eq!(
        store.list_synthesis_packets(&scope).unwrap()[0]
            .required_human_decisions
            .len(),
        2
    );
}

#[test]
fn resolving_winner_gate_asserts_only_the_selected_proposal() {
    let (_dir, store, scope) = initialized_store();
    seed_blocked_evidence(&store, &scope);
    let mut contributions = store.list_contributions(&scope).unwrap();
    let loser_contribution: provenance_core::Contribution =
        serde_json::from_value(serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contribution_comp_time",
            "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
            "participant_slot": "reviewer_b", "stance": "support", "strongest_finding": "Observed alternative",
            "evidence_references": [{"reference_id": "evidence_comp_time", "evidence_type": "source", "summary": "Pinned alternative"}],
            "material_claims": [{"claim_id": "claim_comp_time", "statement": "Observed alternative", "evidence_type": "source", "evidence_reference_ids": ["evidence_comp_time"]}],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
        }))
        .unwrap();
    contributions.push(loser_contribution);
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::contributions_path(&store.layout, &scope),
        &contributions,
    )
    .unwrap();
    let mut packets = store.list_synthesis_packets(&scope).unwrap();
    packets[0].evidence_gaps.clear();
    packets[0].required_human_decisions = serde_json::from_value(serde_json::json!([
        {"decision_key": "pick_overtime_winner", "prompt": "Pick winner", "blocks_promotion": true}
    ]))
    .unwrap();
    packets[0].suggested_artifacts.push(
        serde_json::from_value(serde_json::json!({
            "proposal_id": "proposal_comp_time",
            "proposal_key": "comp_time",
            "proposal_type": "requirement_candidate",
            "summary": "Comp time candidate",
            "origin_participant_slots": ["reviewer_b"]
        }))
        .unwrap(),
    );
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, &scope),
        &packets,
    )
    .unwrap();

    let mut winner = proposal_input(
        &scope,
        "proposal_overtime",
        "Overtime",
        PromotionState::Proposed,
    );
    winner.traceability.supporting_claim_ids = vec![StableId::new("claim_overtime").unwrap()];
    store.create_proposal_card(winner).unwrap();
    let mut loser = proposal_input(
        &scope,
        "proposal_comp_time",
        "Comp time",
        PromotionState::Proposed,
    );
    loser.proposal_key = "comp_time".into();
    loser.traceability.supporting_claim_ids = vec![StableId::new("claim_comp_time").unwrap()];
    store.create_proposal_card(loser).unwrap();

    store
        .assert_proposal_after_human_decision(
            CreateAssertionInput {
                scope_id: scope.clone(),
                id: provenance_core::AssertionId::new("assertion_overtime").unwrap(),
                proposal_id: StableId::new("proposal_overtime").unwrap(),
                synthesis_packet_id: StableId::new("synthesis_overtime").unwrap(),
                supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
            },
            &[StableId::new("pick_overtime_winner").unwrap()],
        )
        .unwrap();

    assert_eq!(store.list_assertion_records(&scope).unwrap().len(), 1);
    assert_eq!(
        store.list_synthesis_packets(&scope).unwrap()[0]
            .suggested_artifacts
            .len(),
        2
    );
}

#[test]
fn repository_actor_allowlist_rejects_unlisted_disposition_actor() {
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
        &crate::shards::assertion_records_path(&store.layout, &scope),
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

    let error = store
        .create_disposition(disposition_input(scope, "forged-reviewer"))
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("no disposition actors configured"),
        "{error}"
    );
}

fn seed_blocked_evidence(store: &crate::state_store::StateStore, scope: &provenance_core::ScopeId) {
    let contribution: provenance_core::Contribution = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contribution_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_overtime", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_overtime", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_overtime"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })).unwrap();
    let synthesis: provenance_core::SynthesisPacket = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "synthesis_overtime",
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

fn allow_actor(store: &crate::state_store::StateStore, id: &str) {
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push(id.into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

fn disposition_input(scope_id: provenance_core::ScopeId, actor_id: &str) -> CreateDispositionInput {
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

fn actor(id: &str) -> DispositionActor {
    DispositionActor {
        identity_type: IdentityType::Human,
        id: id.into(),
        name: None,
    }
}
