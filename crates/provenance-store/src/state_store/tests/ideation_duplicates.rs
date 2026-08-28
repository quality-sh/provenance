use super::initialized_store;
use crate::state_store::IdeationLandingBatch;

#[test]
fn exact_duplicate_proposal_id_is_rejected_at_batch_ingress() {
    let (_dir, store, scope) = initialized_store();
    let initial: IdeationLandingBatch = serde_json::from_value(serde_json::json!({
        "contributions": [], "synthesis_packets": [],
        "proposals": [proposal()], "assertions": [], "dispositions": []
    }))
    .unwrap();
    store
        .land_ideation_batch(None, &scope, initial.clone(), false)
        .unwrap();

    let error = store
        .land_ideation_batch(None, &scope, initial, false)
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("proposal proposal_a already exists"));
}

#[test]
fn exact_duplicate_assertion_id_is_rejected_at_batch_ingress() {
    let (_dir, store, scope) = initialized_store();
    let initial = asserted_batch();
    store
        .land_ideation_batch(None, &scope, initial.clone(), false)
        .unwrap();
    let replay = IdeationLandingBatch {
        contributions: Vec::new(),
        synthesis_packets: Vec::new(),
        proposals: Vec::new(),
        assertions: initial.assertions,
        dispositions: Vec::new(),
    };

    let error = store
        .land_ideation_batch(None, &scope, replay, false)
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("assertion assertion_a already exists"));
}

#[test]
fn exact_duplicate_disposition_id_is_rejected_at_batch_ingress() {
    let (_dir, store, scope) = initialized_store();
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let initial: IdeationLandingBatch = serde_json::from_value(serde_json::json!({
        "contributions": [], "synthesis_packets": [], "proposals": [proposal()],
        "assertions": [],
        "dispositions": [{
            "schema_version": 1, "scope_id": "default", "id": "disposition_a",
            "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Rejected.",
            "actor": {"identity_type": "human", "id": "reviewer"},
            "canonical_artifact": null
        }]
    }))
    .unwrap();
    store
        .land_ideation_batch(None, &scope, initial.clone(), false)
        .unwrap();
    let replay = IdeationLandingBatch {
        contributions: Vec::new(),
        synthesis_packets: Vec::new(),
        proposals: Vec::new(),
        assertions: Vec::new(),
        dispositions: initial.dispositions,
    };

    let error = store
        .land_ideation_batch(None, &scope, replay, false)
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("disposition disposition_a already exists"));
}

fn asserted_batch() -> IdeationLandingBatch {
    serde_json::from_value(serde_json::json!({
        "contributions": [{
            "schema_version": 1, "scope_id": "default", "id": "contribution_a",
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
            "evidence_references": [{"reference_id": "evidence_a", "evidence_type": "source", "summary": "Pinned"}],
            "material_claims": [{"claim_id": "claim_a", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_a"]}],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
        }],
        "synthesis_packets": [{
            "schema_version": 1, "scope_id": "default", "id": "synthesis_a",
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"}, "summary": "Adjudicated",
            "consensus": [], "contested_claims": [], "minority_objections": [], "evidence_gaps": [],
            "unsupported_speculation": [], "open_questions": [],
            "suggested_artifacts": [{"proposal_id": "proposal_a", "proposal_key": "proposal-a", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
            "required_human_decisions": []
        }],
        "proposals": [proposal()],
        "assertions": [{
            "schema_version": 1, "scope_id": "default", "id": "assertion_a",
            "proposal_id": "proposal_a", "synthesis_packet_id": "synthesis_a",
            "supporting_claim_ids": ["claim_a"]
        }],
        "dispositions": []
    }))
    .unwrap()
}

fn proposal() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "proposal_a",
        "proposal_key": "proposal-a", "proposal_type": "requirement_candidate",
        "title": "Candidate", "summary": "Candidate",
        "traceability": {
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": ["claim_a"]
        },
        "promotion_state": "proposed"
    })
}
