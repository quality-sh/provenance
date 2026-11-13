//! Implementation aid, no Rule: the order of the ideation lists after the
//! landings. Each landing record replaces the record with its id or goes at
//! the end, and then the list is in id order.

use super::seeded_requirement_store;
use crate::state_store::{IdeationLandingBatch, StateStore};
use provenance_core::ScopeId;
use serde_json::{json, Value};

#[test]
fn landed_records_are_listed_in_id_order() {
    let (_dir, store, scope) = landing_store();
    land(&store, &scope, batch(&["b"], "First"), false);
    land(&store, &scope, batch(&["c", "a"], "First"), false);

    assert_eq!(listed_ids(&store, &scope), all_kinds(&["a", "b", "c"]));
}

#[test]
fn a_landed_replacement_keeps_the_position_of_its_id() {
    let (_dir, store, scope) = landing_store();
    land(&store, &scope, batch(&["c", "a"], "First"), false);
    let replacement = IdeationLandingBatch {
        proposals: Vec::new(),
        dispositions: Vec::new(),
        ..batch(&["a"], "Second")
    };
    land(&store, &scope, replacement, true);

    let contributions = store.list_contributions(&scope).unwrap();
    let findings: Vec<_> = contributions
        .iter()
        .map(|record| (record.id.as_str(), record.strongest_finding.as_str()))
        .collect();
    assert_eq!(findings, [("contribution_a", "Second"), ("contribution_c", "First")]);
    let packets = store.list_synthesis_packets(&scope).unwrap();
    let summaries: Vec<_> = packets
        .iter()
        .map(|record| (record.id.as_str(), record.summary.as_str()))
        .collect();
    assert_eq!(summaries, [("synthesis_a", "Second"), ("synthesis_c", "First")]);
}

/// A store with the requirement `req_overtime` and the disposition actor
/// `reviewer`.
fn landing_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = seeded_requirement_store();
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    (dir, store, scope)
}

fn land(store: &StateStore, scope: &ScopeId, batch: IdeationLandingBatch, replace: bool) {
    store.land_ideation_batch(scope, batch, replace).unwrap();
}

/// The ids of the records that the test landed, in the order of each list.
/// The seeded store also holds a proposal with a generated id.
fn listed_ids(store: &StateStore, scope: &ScopeId) -> Vec<Vec<String>> {
    fn ids<T>(records: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
        records
            .iter()
            .map(|record| id(record).to_owned())
            .filter(|id| LANDED_KINDS.iter().any(|kind| id.starts_with(kind)))
            .collect()
    }
    vec![
        ids(&store.list_contributions(scope).unwrap(), |r| r.id.as_str()),
        ids(&store.list_synthesis_packets(scope).unwrap(), |r| r.id.as_str()),
        ids(&store.list_proposal_definitions(scope).unwrap(), |r| r.id.as_str()),
        ids(&store.list_dispositions(scope).unwrap(), |r| r.id.as_str()),
    ]
}

const LANDED_KINDS: [&str; 4] = ["contribution", "synthesis", "proposal", "disposition"];

fn all_kinds(suffixes: &[&str]) -> Vec<Vec<String>> {
    LANDED_KINDS
        .iter()
        .map(|kind| suffixes.iter().map(|suffix| format!("{kind}_{suffix}")).collect())
        .collect()
}

fn batch(suffixes: &[&str], text: &str) -> IdeationLandingBatch {
    let rows = |row: fn(&str, &str) -> Value| -> Vec<Value> {
        suffixes.iter().map(|suffix| row(suffix, text)).collect()
    };
    serde_json::from_value(json!({
        "contributions": rows(contribution),
        "synthesis_packets": rows(synthesis_packet),
        "proposals": rows(proposal),
        "assertions": [],
        "dispositions": rows(disposition)
    }))
    .unwrap()
}

fn target() -> Value {
    json!({"artifact_type": "requirement", "artifact_id": "req_overtime"})
}

fn contribution(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("contribution_{suffix}"), "target": target(),
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": text,
        "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
        "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    })
}

fn synthesis_packet(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("synthesis_{suffix}"), "target": target(),
        "summary": text, "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [], "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [], "required_human_decisions": []
    })
}

fn proposal(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("proposal_{suffix}"),
        "proposal_key": format!("proposal-{suffix}"), "proposal_type": "requirement_candidate",
        "title": text, "summary": text,
        "traceability": {
            "target": target(), "source_ids": [], "evidence_references": [],
            "supporting_claim_ids": ["claim_a"]
        },
        "promotion_state": "proposed"
    })
}

fn disposition(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("disposition_{suffix}"),
        "proposal_id": format!("proposal_{suffix}"), "decision": "rejected", "rationale": text,
        "actor": {"identity_type": "human", "id": "reviewer"}, "canonical_artifact": null
    })
}
