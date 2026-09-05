//! A new contribution or synthesis packet must name a record that exists in
//! its scope. The check runs in the four ideation writers only; it never
//! runs on a read.

use super::seeded_requirement_store;
use crate::state_store::{
    CreateContributionInput, CreateDomainInput, CreateSynthesisPacketInput, StateStore,
};
use provenance_core::{
    ContributionStance, IdeationTarget, IdeationTargetType, ScopeId, StableId, UncertaintyLevel,
    UncertaintyRating,
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

pub(super) fn contribution_on(
    scope: &ScopeId,
    id: &str,
    target: IdeationTarget,
) -> CreateContributionInput {
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

pub(super) fn synthesis_on(
    scope: &ScopeId,
    id: &str,
    target: IdeationTarget,
) -> CreateSynthesisPacketInput {
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
