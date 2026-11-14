//! The ideation records of one complete proposal lifecycle, for the import tests.

/// Adds a contribution, a synthesis packet, a proposal, an assertion and an
/// accepted disposition to the scope export `lifecycle`. Each record is about
/// the Source `source_a`, and the disposition actor is `reviewer`.
pub fn add_modern_lifecycle(lifecycle: &mut serde_json::Value) {
    lifecycle["contributions"] = serde_json::json!([{
        "scope_id": "default", "id": "contribution_a",
        "target": {"artifact_type": "source", "artifact_id": "source_a"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_a", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_a", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_a"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    }]);
    lifecycle["synthesis_packets"] = serde_json::json!([{
        "scope_id": "default", "id": "synthesis_a",
        "target": {"artifact_type": "source", "artifact_id": "source_a"}, "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [], "evidence_gaps": [],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_a", "proposal_key": "proposal-a", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    }]);
    lifecycle["proposal_cards"] = serde_json::json!([{
        "scope_id": "default", "id": "proposal_a", "proposal_key": "proposal-a",
        "proposal_type": "requirement_candidate", "title": "Candidate", "summary": "Candidate",
        "traceability": {"target": {"artifact_type": "source", "artifact_id": "source_a"}, "source_ids": ["source_a"], "evidence_references": [], "supporting_claim_ids": ["claim_a"]},
        "promotion_state": "proposed"
    }]);
    lifecycle["assertion_records"] = serde_json::json!([{
        "scope_id": "default", "id": "assertion_a", "proposal_id": "proposal_a",
        "synthesis_packet_id": "synthesis_a", "supporting_claim_ids": ["claim_a"]
    }]);
    lifecycle["dispositions"] = serde_json::json!([{
        "scope_id": "default", "id": "disposition_a", "proposal_id": "proposal_a",
        "decision": "accepted", "rationale": "Reviewed", "actor": {"identity_type": "human", "id": "reviewer"},
        "canonical_artifact": {"artifact_type": "source", "artifact_id": "source_a"},
        "external_action": {"system": "github", "scope": "acme/payroll", "kind": "deployment", "key": "prod-44"}
    }]);
}
