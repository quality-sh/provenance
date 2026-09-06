//! A contribution or synthesis packet already in the scope whose target
//! names no record is a gap, reported with the wording of the other
//! dangling references, and refuses nothing.

use super::super::{compute_gaps, find_gaps, GapGraph, GapItem, GapKind};
use super::fixtures::{domain, requirement};
use crate::cache::tests::fixtures::{append_record, seeded_layout};
use crate::state_store::{CreateContributionInput, StateStore};
use provenance_core::{
    Boundary, Contribution, ContributionStance, IdeationTarget, IdeationTargetType, NodeType,
    ScopeId, StableId, SynthesisPacket, UncertaintyLevel, UncertaintyRating,
    SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;
use serde_json::json;

fn contribution_json(id: &str, kind: &str, target_id: &str) -> serde_json::Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": id,
        "target": {"artifact_type": kind, "artifact_id": target_id},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
        "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })
}

fn synthesis_json(id: &str, kind: &str, target_id: &str) -> serde_json::Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": id,
        "target": {"artifact_type": kind, "artifact_id": target_id}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [], "evidence_gaps": [],
        "unsupported_speculation": [], "open_questions": [], "suggested_artifacts": [],
        "required_human_decisions": []
    })
}

fn contribution(id: &str, kind: &str, target_id: &str) -> Contribution {
    serde_json::from_value(contribution_json(id, kind, target_id)).unwrap()
}

fn synthesis_packet(id: &str, kind: &str, target_id: &str) -> SynthesisPacket {
    serde_json::from_value(synthesis_json(id, kind, target_id)).unwrap()
}

fn boundary(id: &str) -> Boundary {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": id,
        "requirement_id": "req_overtime", "statement": "Back pay is out of scope"
    }))
    .unwrap()
}

fn gaps_with(
    contributions: &[Contribution],
    synthesis_packets: &[SynthesisPacket],
) -> Vec<GapItem> {
    let scope = ScopeId::new("default").unwrap();
    compute_gaps(&GapGraph {
        scope: &scope,
        sources: &[],
        requirements: &[requirement("req_overtime")],
        resolutions: &[],
        rules: &[],
        topics: &[],
        questions: &[],
        threads: &[],
        domains: &[domain("domain_payroll")],
        boundaries: &[boundary("boundary_no_backpay")],
        contributions,
        synthesis_packets,
    })
}

fn dangling(gaps: &[GapItem]) -> Vec<&GapItem> {
    gaps.iter()
        .filter(|gap| gap.kind == GapKind::DanglingReference)
        .collect()
}

#[test]
#[verifies("rule_old_dangling_ideation_target_is_a_gap", examples)]
fn a_contribution_on_a_missing_target_is_a_dangling_reference() {
    let gaps = gaps_with(
        &[contribution("contrib_old", "requirement", "req_gone")],
        &[],
    );
    let found = dangling(&gaps);
    assert_eq!(
        found.len(),
        1,
        "one gap for the one missing target: {gaps:?}"
    );
    assert_eq!(found[0].node_type, NodeType::Requirement);
    assert_eq!(found[0].node_id, "req_gone");
    assert_eq!(
        found[0].reason,
        "contribution contrib_old target points at missing requirement req_gone"
    );
}

#[test]
#[verifies("rule_old_dangling_ideation_target_is_a_gap", examples)]
fn a_synthesis_packet_on_a_missing_target_is_a_dangling_reference() {
    let gaps = gaps_with(
        &[],
        &[synthesis_packet("synth_old", "question", "question_gone")],
    );
    let found = dangling(&gaps);
    assert_eq!(
        found.len(),
        1,
        "one gap for the one missing target: {gaps:?}"
    );
    assert_eq!(found[0].node_type, NodeType::Question);
    assert_eq!(
        found[0].reason,
        "synthesis packet synth_old target points at missing question question_gone"
    );
}

#[test]
#[verifies("rule_old_dangling_ideation_target_is_a_gap", examples)]
fn a_target_that_exists_is_not_a_gap() {
    let gaps = gaps_with(
        &[
            contribution("contrib_a", "domain", "domain_payroll"),
            contribution("contrib_b", "boundary", "boundary_no_backpay"),
        ],
        &[synthesis_packet("synth_a", "requirement", "req_overtime")],
    );
    assert!(
        dangling(&gaps).is_empty(),
        "an existing target is not reported: {gaps:?}"
    );
}

#[test]
#[verifies("rule_old_dangling_ideation_target_is_a_gap", examples)]
fn a_missing_boundary_target_is_a_dangling_reference() {
    let gaps = gaps_with(
        &[contribution("contrib_old", "boundary", "boundary_gone")],
        &[],
    );
    let found = dangling(&gaps);
    assert_eq!(found.len(), 1, "{gaps:?}");
    assert_eq!(found[0].node_type, NodeType::Boundary);
    assert_eq!(
        found[0].reason,
        "contribution contrib_old target points at missing boundary boundary_gone"
    );
}

/// The store holds a contribution and a synthesis packet written before the
/// write-time check existed. The gap report names both, and the next valid
/// write and the next read go through.
#[test]
#[verifies("rule_old_dangling_ideation_target_is_a_gap", examples)]
fn an_existing_dangling_target_is_a_gap_not_a_refusal() {
    let (_dir, layout, scope) = seeded_layout();
    append_record(
        &crate::shards::contributions_path(&layout, &scope),
        &contribution_json("contrib_old", "requirement", "req_gone"),
    );
    append_record(
        &crate::shards::synthesis_packets_path(&layout, &scope),
        &synthesis_json("synth_old", "rule", "rule_gone"),
    );

    let gaps = find_gaps(&layout, &scope).unwrap();
    let reasons: Vec<&str> = dangling(&gaps)
        .iter()
        .map(|gap| gap.reason.as_str())
        .collect();
    assert_eq!(
        reasons,
        [
            "contribution contrib_old target points at missing requirement req_gone",
            "synthesis packet synth_old target points at missing rule rule_gone",
        ]
    );

    let store = StateStore::new(layout);
    store
        .create_contribution(CreateContributionInput {
            scope_id: scope.clone(),
            id: StableId::new("contrib_new").unwrap(),
            target: IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: StableId::new("req_schads_overtime").unwrap(),
            },
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
        })
        .expect("an old dangling target refuses no new valid write");
    let ids: Vec<String> = store
        .list_contributions(&scope)
        .unwrap()
        .iter()
        .map(|record| record.id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["contrib_new", "contrib_old"]);
}
