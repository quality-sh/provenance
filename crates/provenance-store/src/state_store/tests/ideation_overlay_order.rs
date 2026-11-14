//! Implementation aid, no Rule: the order of the ideation lists after the
//! landings. Each landing record replaces the record with its id or goes at
//! the end, and then the list is in id order. The scope checks and the scope
//! ingestion read the same lists; this test examines only the list readers.

use super::seeded_requirement_store;
use crate::state_store::{IdeationLandingBatch, StateStore};
use provenance_core::ScopeId;
use serde_json::{json, Value};

#[test]
fn landed_records_and_file_rows_are_listed_in_id_order() {
    let (_dir, store, scope) = landing_store();
    let file_rows = listed_ids(&store, &scope);
    assert_eq!(
        file_rows.iter().map(Vec::len).collect::<Vec<_>>(),
        [0, 0, 1, 0, 0],
        "the seeded store holds one proposal in its file"
    );
    land(&store, &scope, batch(&["b"], "First"), false);
    land(&store, &scope, batch(&["c", "a"], "First"), false);

    let mut expected = all_kinds(&["a", "b", "c"]);
    for (list, rows) in expected.iter_mut().zip(file_rows) {
        list.extend(rows);
        list.sort();
    }
    assert_eq!(listed_ids(&store, &scope), expected);
}

#[test]
fn a_landed_replacement_replaces_its_id_and_the_sort_sets_its_place() {
    let (_dir, store, scope) = landing_store();
    let first = IdeationLandingBatch {
        assertions: Vec::new(),
        ..batch(&["c", "a"], "First")
    };
    land(&store, &scope, first, false);
    let replacement = IdeationLandingBatch {
        proposals: Vec::new(),
        assertions: Vec::new(),
        dispositions: Vec::new(),
        ..batch(&["a"], "Second")
    };
    land(&store, &scope, replacement, true);

    let contributions = store.list_contributions(&scope).unwrap();
    let findings: Vec<_> = contributions
        .iter()
        .map(|record| (record.id.as_str(), record.strongest_finding.as_str()))
        .collect();
    assert_eq!(
        findings,
        [("contribution_a", "Second"), ("contribution_c", "First")]
    );
    let packets = store.list_synthesis_packets(&scope).unwrap();
    let summaries: Vec<_> = packets
        .iter()
        .map(|record| (record.id.as_str(), record.summary.as_str()))
        .collect();
    assert_eq!(
        summaries,
        [("synthesis_a", "Second"), ("synthesis_c", "First")]
    );
}

/// A store with the requirement `req_overtime` and the disposition actor
/// `reviewer`.
fn landing_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = seeded_requirement_store();
    crate::test_support::allow_reviewer(&store.layout);
    (dir, store, scope)
}

fn land(store: &StateStore, scope: &ScopeId, batch: IdeationLandingBatch, replace: bool) {
    store.land_ideation_batch(scope, batch, replace).unwrap();
}

/// The ids of each ideation list, in the order of the list.
fn listed_ids(store: &StateStore, scope: &ScopeId) -> Vec<Vec<String>> {
    fn ids<T>(records: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
        records.iter().map(|record| id(record).to_owned()).collect()
    }
    vec![
        ids(&store.list_contributions(scope).unwrap(), |r| r.id.as_str()),
        ids(&store.list_synthesis_packets(scope).unwrap(), |r| {
            r.id.as_str()
        }),
        ids(&store.list_proposal_definitions(scope).unwrap(), |r| {
            r.id.as_str()
        }),
        ids(&store.list_assertion_records(scope).unwrap(), |r| {
            r.id.as_str()
        }),
        ids(&store.list_dispositions(scope).unwrap(), |r| r.id.as_str()),
    ]
}

fn all_kinds(suffixes: &[&str]) -> Vec<Vec<String>> {
    [
        "contribution",
        "synthesis",
        "proposal",
        "assertion",
        "disposition",
    ]
    .iter()
    .map(|kind| {
        suffixes
            .iter()
            .map(|suffix| format!("{kind}_{suffix}"))
            .collect()
    })
    .collect()
}

/// One record of each kind for each of `suffixes`. The assertion of a suffix
/// rests on the claim of its contribution and on its synthesis packet.
fn batch(suffixes: &[&str], text: &str) -> IdeationLandingBatch {
    let rows = |row: fn(&str, &str) -> Value| -> Vec<Value> {
        suffixes.iter().map(|suffix| row(suffix, text)).collect()
    };
    serde_json::from_value(json!({
        "contributions": rows(contribution),
        "synthesis_packets": rows(synthesis_packet),
        "proposals": rows(proposal),
        "assertions": rows(assertion),
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
        "evidence_references": [{
            "reference_id": format!("evidence_{suffix}"), "evidence_type": "source",
            "summary": "Pinned"
        }],
        "material_claims": [{
            "claim_id": format!("claim_{suffix}"), "statement": "Observed",
            "evidence_type": "source", "evidence_reference_ids": [format!("evidence_{suffix}")]
        }],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"},
        "open_questions": []
    })
}

fn synthesis_packet(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("synthesis_{suffix}"), "target": target(),
        "summary": text, "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [], "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{
            "proposal_id": format!("proposal_{suffix}"),
            "proposal_key": format!("proposal-{suffix}"),
            "proposal_type": "requirement_candidate", "summary": "Candidate",
            "origin_participant_slots": ["reviewer"]
        }],
        "required_human_decisions": []
    })
}

fn proposal(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("proposal_{suffix}"),
        "proposal_key": format!("proposal-{suffix}"), "proposal_type": "requirement_candidate",
        "title": text, "summary": text,
        "traceability": {
            "target": target(), "source_ids": [], "evidence_references": [],
            "supporting_claim_ids": [format!("claim_{suffix}")]
        },
        "promotion_state": "proposed"
    })
}

fn assertion(suffix: &str, _text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("assertion_{suffix}"),
        "proposal_id": format!("proposal_{suffix}"),
        "synthesis_packet_id": format!("synthesis_{suffix}"),
        "supporting_claim_ids": [format!("claim_{suffix}")]
    })
}

fn disposition(suffix: &str, text: &str) -> Value {
    json!({
        "scope_id": "default", "id": format!("disposition_{suffix}"),
        "proposal_id": format!("proposal_{suffix}"), "decision": "rejected", "rationale": text,
        "actor": {"identity_type": "human", "id": "reviewer"}, "canonical_artifact": null
    })
}
