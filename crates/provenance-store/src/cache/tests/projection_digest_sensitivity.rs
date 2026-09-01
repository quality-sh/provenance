use super::super::*;
use super::fixtures::*;
use super::projection_stamp_behavior::seed_integration_shards;
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::ScopeId;
use serde_json::json;

/// One fresh record for the family, as the JSON its shard holds.
///
/// References point at records the seeded fixture already holds; nothing
/// here has to satisfy write-path validation, only the family's parse.
fn fresh_record(family: ProjectionFamily, scope: &str) -> serde_json::Value {
    let target = json!({"artifact_type": "requirement", "artifact_id": "req_schads_overtime"});
    match family.family_name() {
        "sources" => json!({"schema_version": 1, "scope_id": scope, "id": "source_new",
            "name": "New", "source_type": "policy", "url": null}),
        "domains" => json!({"schema_version": 1, "scope_id": scope, "id": "domain_new",
            "name": "New"}),
        "requirements" => json!({"schema_version": 1, "scope_id": scope, "id": "req_new",
            "statement": "New", "status": "active"}),
        "boundaries" => json!({"schema_version": 1, "scope_id": scope, "id": "boundary_new",
            "requirement_id": "req_schads_overtime", "statement": "New"}),
        "topics" => json!({"schema_version": 1, "scope_id": scope, "id": "topic_new",
            "requirement_id": "req_schads_overtime", "title": "New", "status": "open"}),
        "questions" => json!({"schema_version": 1, "scope_id": scope, "id": "question_new",
            "topic_id": "topic_new", "requirement_id": "req_schads_overtime",
            "question": "New?", "resolution_method": "grill", "status": "open"}),
        "edges" => json!({"schema_version": 1, "scope_id": scope, "id": "edge_new",
            "edge_type": "references", "from_type": "requirement",
            "from_id": "req_schads_overtime", "to_type": "source", "to_id": "source_schads"}),
        "resolutions" => json!({"schema_version": 1, "scope_id": scope, "id": "res_new",
            "title": "New", "position": "P", "rationale": "R", "status": "proposed"}),
        "rules" => json!({"schema_version": 1, "scope_id": scope, "id": "rule_new",
            "statement": "New", "status": "active", "severity": "high"}),
        "threads" => json!({"schema_version": 1, "scope_id": scope, "id": "thread_new",
            "parent": {"node_type": "requirement", "node_id": "req_schads_overtime"},
            "status": "active", "created_at": 1}),
        "messages" => json!({"schema_version": 1, "scope_id": scope, "id": "message_new",
            "thread_id": "thread_new", "role": "user", "body": "New", "created_at": 1}),
        "contributions" => json!({"schema_version": 1, "scope_id": scope, "id": "contribution_new",
            "target": target, "participant_slot": "slot_a", "stance": "support",
            "strongest_finding": "F", "evidence_references": [], "material_claims": [],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "R"}, "open_questions": []}),
        "synthesis_packets" => json!({"schema_version": 1, "scope_id": scope, "id": "packet_new",
            "target": target, "summary": "S", "consensus": [], "contested_claims": [],
            "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
            "open_questions": [], "suggested_artifacts": [], "required_human_decisions": []}),
        "proposal_cards" => json!({"schema_version": 1, "scope_id": scope, "id": "proposal_new",
            "proposal_key": "key_new", "proposal_type": "requirement_candidate",
            "title": "T", "summary": "S",
            "traceability": {"target": target, "source_ids": [],
                "evidence_references": [], "supporting_claim_ids": []},
            "promotion_state": "proposed"}),
        "assertion_records" => {
            json!({"schema_version": 1, "scope_id": scope, "id": "assertion_new",
            "proposal_id": "proposal_new", "synthesis_packet_id": "packet_new",
            "supporting_claim_ids": ["claim_a"]})
        }
        "dispositions" => json!({"schema_version": 1, "scope_id": scope, "id": "disposition_new",
            "proposal_id": "proposal_new", "decision": "rejected", "rationale": "R",
            "actor": {"identity_type": "human", "id": "reviewer"}}),
        "implementation_bindings" => json!({"schema_version": 1, "scope_id": scope,
            "id": "bind_impl_new", "rule_id": "rule_schads_pay_001", "declared_by": "agent",
            "file": "src/new.rs", "symbol": "new"}),
        "verification_bindings" => json!({"schema_version": 1, "scope_id": scope,
            "id": "bind_ver_new", "rule_id": "rule_schads_pay_001", "key": "key_new",
            "method": "examples", "declared_by": "agent", "file": "tests/new.rs"}),
        "requirement_reviews" => json!({"schema_version": 1, "scope_id": scope,
            "id": "review_new", "rule_id": "rule_schads_pay_001",
            "requirement_id": "req_schads_overtime", "field": "statement",
            "before": "Old", "after": "New", "changed_at": 2}),
        other => panic!("no fresh record for family {other}"),
    }
}

/// A layout whose ideation tier is a valid aggregate, so listing survives
/// the same validation materialization runs: one contribution with a claim,
/// one qualifying synthesis packet, one proposal resting on the claim, one
/// assertion, and one deferred disposition by an allowed actor.
fn aggregate_layout() -> (tempfile::TempDir, ProvenanceLayout, ScopeId) {
    let (dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());

    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let scope_name = scope.as_str();
    let seed = |family: ProjectionFamily, record: serde_json::Value| {
        let path = family.shard_path(&layout, Some(&scope)).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{record}\n")).unwrap();
    };
    seed(
        ProjectionFamily::Contributions,
        json!({"schema_version": 1, "scope_id": scope_name, "id": "contribution_base",
            "target": {"artifact_type": "requirement", "artifact_id": "req_schads_overtime"},
            "participant_slot": "reviewer", "stance": "support",
            "strongest_finding": "Observed",
            "evidence_references": [{"reference_id": "evidence_base",
                "evidence_type": "source", "summary": "Pinned"}],
            "material_claims": [{"claim_id": "claim_base", "statement": "Observed",
                "evidence_type": "source", "evidence_reference_ids": ["evidence_base"]}],
            "risks": [], "objections": [], "challenges": [],
            "suggested_artifact_changes": [], "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []}),
    );
    seed(
        ProjectionFamily::SynthesisPackets,
        json!({"schema_version": 1, "scope_id": scope_name, "id": "packet_base",
            "target": {"artifact_type": "requirement", "artifact_id": "req_schads_overtime"},
            "summary": "Adjudicated", "consensus": [], "contested_claims": [],
            "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
            "open_questions": [],
            "suggested_artifacts": [{"proposal_id": "proposal_base", "proposal_key": "base",
                "proposal_type": "requirement_candidate", "summary": "Candidate",
                "origin_participant_slots": ["reviewer"]}],
            "required_human_decisions": []}),
    );
    seed(
        ProjectionFamily::ProposalCards,
        json!({"schema_version": 1, "scope_id": scope_name, "id": "proposal_base",
            "proposal_key": "base", "proposal_type": "requirement_candidate",
            "title": "Base", "summary": "Base",
            "traceability": {"target": {"artifact_type": "requirement",
                "artifact_id": "req_schads_overtime"}, "source_ids": [],
                "evidence_references": [], "supporting_claim_ids": ["claim_base"]},
            "promotion_state": "proposed"}),
    );
    seed(
        ProjectionFamily::AssertionRecords,
        json!({"schema_version": 1, "scope_id": scope_name, "id": "assertion_base",
            "proposal_id": "proposal_base", "synthesis_packet_id": "packet_base",
            "supporting_claim_ids": ["claim_base"]}),
    );
    seed(
        ProjectionFamily::Dispositions,
        json!({"schema_version": 1, "scope_id": scope_name, "id": "disposition_base",
            "proposal_id": "proposal_base", "decision": "deferred",
            "rationale": "Deferred for now",
            "actor": {"identity_type": "human", "id": "reviewer"}}),
    );
    (dir, layout, scope)
}

/// Changes exactly one record of the family, keeping the aggregate valid.
///
/// Most families take one appended record. An assertion admits no free
/// text and no second assertion for the same proposal, so its id moves; a
/// disposition's rationale moves for the same reason.
fn change_one_record(layout: &ProvenanceLayout, family: ProjectionFamily, scope: &ScopeId) {
    let shard_scope = family.is_scoped().then_some(scope);
    let path = family.shard_path(layout, shard_scope).unwrap();
    match family {
        ProjectionFamily::AssertionRecords => {
            let content = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, content.replace("assertion_base", "assertion_moved")).unwrap();
        }
        ProjectionFamily::Dispositions => {
            let content = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, content.replace("Deferred for now", "Deferred again")).unwrap();
        }
        family => {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
            existing.push_str(&fresh_record(family, scope.as_str()).to_string());
            existing.push('\n');
            std::fs::write(path, existing).unwrap();
        }
    }
}

fn current_digest(store: &StateStore, scopes: &[ScopeId]) -> String {
    revision_digest(&family_content_digests(store, scopes).unwrap()).unwrap()
}

#[test]
fn one_changed_record_in_any_family_moves_the_revision_digest() {
    for family in ProjectionFamily::ALL {
        let (_dir, layout, scope) = aggregate_layout();
        let store = StateStore::new(layout.clone());
        let scopes = vec![scope.clone()];
        let before = current_digest(&store, &scopes);
        change_one_record(&layout, family, &scope);
        let after = current_digest(&store, &scopes);
        assert_ne!(
            before,
            after,
            "family `{}` must move the revision digest",
            family.family_name()
        );
    }
}

#[test]
fn an_ideation_change_leaves_every_other_family_digest_standing() {
    let (_dir, layout, scope) = aggregate_layout();
    let store = StateStore::new(layout.clone());
    let scopes = vec![scope.clone()];
    let before = family_content_digests(&store, &scopes).unwrap();
    change_one_record(&layout, ProjectionFamily::Contributions, &scope);
    let after = family_content_digests(&store, &scopes).unwrap();

    assert_ne!(
        revision_digest(&before).unwrap(),
        revision_digest(&after).unwrap(),
        "the revision digest covers ideation records"
    );
    for (old, new) in before.iter().zip(after.iter()) {
        assert_eq!(old.family, new.family);
        if old.family == "contributions" {
            assert_ne!(old.digest, new.digest);
        } else {
            assert_eq!(old.digest, new.digest, "family `{}` moved", old.family);
        }
    }
}
