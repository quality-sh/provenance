//! A new contribution or synthesis packet must name a record that exists in
//! its scope. The check runs in the ideation writers only, the direct ones
//! and the landed batch; it never runs on a read.

use super::seeded_requirement_store;
use crate::state_store::{
    CreateBoundaryInput, CreateContributionInput, CreateDomainInput, CreateSynthesisPacketInput,
    IdeationLandingBatch, StateStore,
};
use provenance_core::{
    ContributionStance, IdeationTarget, IdeationTargetType, ScopeId, StableId, UncertaintyLevel,
    UncertaintyRating, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn target(kind: IdeationTargetType, id: &str) -> IdeationTarget {
    IdeationTarget {
        artifact_type: kind,
        artifact_id: sid(id),
    }
}

fn contribution_on(scope: &ScopeId, id: &str, target: IdeationTarget) -> CreateContributionInput {
    CreateContributionInput {
        scope_id: scope.clone(),
        id: sid(id),
        target,
        participant_slot: "reviewer".into(),
        stance: ContributionStance::Support,
        strongest_finding: "Observed".into(),
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

fn synthesis_on(scope: &ScopeId, id: &str, target: IdeationTarget) -> CreateSynthesisPacketInput {
    CreateSynthesisPacketInput {
        scope_id: scope.clone(),
        id: sid(id),
        target,
        summary: "Adjudicated".into(),
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

fn create_domain(store: &StateStore, scope: &ScopeId, id: &str) {
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: sid(id),
            name: id.into(),
            description: None,
            color: None,
        })
        .unwrap();
}

#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_contribution_naming_a_missing_target_is_refused() {
    let (_dir, store, scope) = seeded_requirement_store();
    let missing = target(IdeationTargetType::Requirement, "req_gone");
    let error = store
        .create_contribution(contribution_on(&scope, "contrib_a", missing.clone()))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("req_gone") && error.contains("requirement"),
        "the refusal names the missing record: {error}"
    );
    let error = store
        .upsert_contribution(contribution_on(&scope, "contrib_a", missing))
        .unwrap_err()
        .to_string();
    assert!(error.contains("req_gone"), "upsert refuses too: {error}");
    assert!(
        store.list_contributions(&scope).unwrap().is_empty(),
        "a refused contribution is not stored"
    );
}

#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_synthesis_packet_naming_a_missing_target_is_refused() {
    let (_dir, store, scope) = seeded_requirement_store();
    let missing = target(IdeationTargetType::Question, "question_gone");
    let error = store
        .create_synthesis_packet(synthesis_on(&scope, "synth_a", missing.clone()))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("question_gone") && error.contains("question"),
        "the refusal names the missing record: {error}"
    );
    let error = store
        .upsert_synthesis_packet(synthesis_on(&scope, "synth_a", missing))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("question_gone"),
        "upsert refuses too: {error}"
    );
    assert!(store.list_synthesis_packets(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_boundary_target_is_checked_like_the_other_kinds() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: sid("boundary_no_backpay"),
            requirement_id: sid("req_overtime"),
            statement: "Back pay is out of scope".into(),
            source_ref: None,
        })
        .unwrap();
    let present = target(IdeationTargetType::Boundary, "boundary_no_backpay");
    store
        .create_contribution(contribution_on(&scope, "contrib_a", present.clone()))
        .unwrap();
    store
        .create_synthesis_packet(synthesis_on(&scope, "synth_a", present))
        .unwrap();

    let missing = target(IdeationTargetType::Boundary, "boundary_gone");
    let error = store
        .create_contribution(contribution_on(&scope, "contrib_b", missing.clone()))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("boundary boundary_gone"),
        "the refusal names the missing boundary: {error}"
    );
    let error = store
        .create_synthesis_packet(synthesis_on(&scope, "synth_b", missing))
        .unwrap_err()
        .to_string();
    assert!(error.contains("boundary boundary_gone"), "{error}");
}

/// The landed batch is the third writer of each kind, reached through
/// `provenance swarm-backtrace land`.
#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_landed_batch_naming_a_missing_target_is_refused() {
    let (_dir, store, scope) = seeded_requirement_store();
    let contribution = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contrib_landed",
        "target": {"artifact_type": "requirement", "artifact_id": "req_gone"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
        "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    }))
    .unwrap();
    let synthesis_packet = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "synth_landed",
        "target": {"artifact_type": "question", "artifact_id": "question_gone"},
        "summary": "Adjudicated", "consensus": [], "contested_claims": [],
        "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
        "open_questions": [], "suggested_artifacts": [], "required_human_decisions": []
    }))
    .unwrap();
    for (contributions, synthesis_packets, missing) in [
        (vec![contribution], Vec::new(), "requirement req_gone"),
        (Vec::new(), vec![synthesis_packet], "question question_gone"),
    ] {
        let error = store
            .land_ideation_batch(
                &scope,
                IdeationLandingBatch {
                    contributions,
                    synthesis_packets,
                    proposals: Vec::new(),
                    assertions: Vec::new(),
                    dispositions: Vec::new(),
                },
                false,
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains(missing), "{error}");
    }
    assert!(store.list_contributions(&scope).unwrap().is_empty());
    assert!(store.list_synthesis_packets(&scope).unwrap().is_empty());
    assert!(store.list_ideation_landings(&scope).unwrap().is_empty());
}

/// An old dangling target refuses nothing, including a rewrite of its own
/// record: an upsert or a re-landed batch that keeps the stored target goes
/// through, and only a target the record did not carry before is checked.
#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_rewrite_that_keeps_an_old_dangling_target_is_not_refused() {
    let (_dir, store, scope) = seeded_requirement_store();
    let old = target(IdeationTargetType::Requirement, "req_gone");
    crate::cache::tests::fixtures::append_record(
        &crate::shards::contributions_path(&store.layout, &scope),
        &serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contrib_old",
            "target": {"artifact_type": "requirement", "artifact_id": "req_gone"},
            "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
            "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
            "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
        }),
    );

    store
        .upsert_contribution(contribution_on(&scope, "contrib_old", old))
        .expect("the stored target is kept, so nothing new is checked");
    let stored = store.list_contributions(&scope).unwrap();
    store
        .land_ideation_batch(
            &scope,
            IdeationLandingBatch {
                contributions: stored,
                synthesis_packets: Vec::new(),
                proposals: Vec::new(),
                assertions: Vec::new(),
                dispositions: Vec::new(),
            },
            true,
        )
        .expect("re-landing the record unchanged goes through");

    let moved = target(IdeationTargetType::Requirement, "req_also_gone");
    let error = store
        .upsert_contribution(contribution_on(&scope, "contrib_old", moved))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("req_also_gone"),
        "a new target is checked: {error}"
    );
}

/// The index reads the unfiltered shards, as every other writer does, so
/// a target at a retired record is accepted; the gap pass drops it on the
/// same terms as a reference field. Refusing it here would make the
/// ideation writers stricter than the graph writers.
#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_target_at_a_retired_record_is_accepted() {
    let (_dir, store, scope) = seeded_requirement_store();
    crate::cache::tests::fixtures::rewrite_records(
        &crate::shards::requirements_path(&store.layout, &scope),
        |record| record["retired"] = serde_json::Value::Bool(true),
    );
    let retired = target(IdeationTargetType::Requirement, "req_overtime");
    store
        .create_contribution(contribution_on(&scope, "contrib_a", retired.clone()))
        .unwrap();
    store
        .create_synthesis_packet(synthesis_on(&scope, "synth_a", retired))
        .unwrap();
    assert_eq!(store.list_contributions(&scope).unwrap().len(), 1);
    assert_eq!(store.list_synthesis_packets(&scope).unwrap().len(), 1);
}

#[test]
#[verifies("rule_new_ideation_target_names_a_record", examples)]
fn a_target_of_a_non_canonical_kind_is_found_when_it_exists() {
    let (_dir, store, scope) = seeded_requirement_store();
    create_domain(&store, &scope, "domain_payroll");
    let domain = target(IdeationTargetType::Domain, "domain_payroll");
    store
        .create_contribution(contribution_on(&scope, "contrib_a", domain.clone()))
        .unwrap();
    store
        .create_synthesis_packet(synthesis_on(&scope, "synth_a", domain))
        .unwrap();
    assert_eq!(store.list_contributions(&scope).unwrap().len(), 1);
    assert_eq!(store.list_synthesis_packets(&scope).unwrap().len(), 1);
}
