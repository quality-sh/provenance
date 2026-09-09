//! Fixtures shared by the proposal-lifecycle dispatch tests. The tests live
//! beside the behavior they pin: `lists` pins reads and the effective-state
//! projection, `writes` pins creations, refusals, and publication behavior.
use super::super::{PreparedContext, PreparedScope};
use crate::state_store::{
    CreateAssertionInput, CreateDispositionInput, CreateProposalCardInput, CreateRequirementInput,
    StateStore,
};
use crate::{
    layout::ProvenanceLayout,
    write_error::{WriteError, WriteFailure},
};
use provenance_core::{
    protocol::failure::OperationError, AssertionId, DispositionActor, DispositionDecision,
    IdeationTarget, IdeationTargetType, IdentityType, Manifest, PromotionState,
    ProposalTraceability, ProposalType, RepoPathPrefix, ScopeId, StableId,
};
use serde_json::json;

mod lists;
mod writes;

fn initialized() -> (tempfile::TempDir, StateStore, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (dir, StateStore::new(layout), scope)
}

/// One native repository and one repository reached only through the catalog
/// seam, so every test compares the two executions of the same request.
fn twin() -> (
    tempfile::TempDir,
    StateStore,
    ScopeId,
    tempfile::TempDir,
    StateStore,
    ScopeId,
) {
    let (native_dir, native_store, native_scope) = initialized();
    let (dir, store, scope) = initialized();
    (native_dir, native_store, native_scope, dir, store, scope)
}

fn prepared(dir: &tempfile::TempDir, scope: &ScopeId) -> PreparedContext {
    PreparedContext::for_scope(PreparedScope {
        root: camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap(),
        scope: scope.clone(),
        requested_target: "selected".into(),
    })
}

fn proposal_input(scope: &ScopeId, id: &str) -> CreateProposalCardInput {
    CreateProposalCardInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        proposal_key: "overtime".into(),
        proposal_type: ProposalType::RequirementCandidate,
        title: "Overtime".into(),
        summary: "Clarify the overtime requirement.".into(),
        confidence: None,
        traceability: ProposalTraceability {
            target: IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: StableId::new("req_overtime").unwrap(),
            },
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            supporting_claim_ids: Vec::new(),
        },
        builds_on: Vec::new(),
        promotion_state: PromotionState::Proposed,
        duplicate_of: None,
        superseded_by: None,
    }
}

fn actor(id: &str) -> DispositionActor {
    DispositionActor {
        identity_type: IdentityType::Human,
        id: id.into(),
        name: None,
    }
}

fn rejected_disposition(scope: &ScopeId, id: &str, proposal_id: &str) -> CreateDispositionInput {
    CreateDispositionInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        proposal_id: StableId::new(proposal_id).unwrap(),
        decision: DispositionDecision::Rejected,
        rationale: "Reviewed".into(),
        actor: actor("reviewer"),
        canonical_artifact: None,
        external_action: None,
    }
}

fn seed_requirement(store: &StateStore, scope: &ScopeId) {
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_overtime").unwrap(),
            statement: "Overtime".into(),
            description: None,
            status: provenance_core::RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

/// Allow one disposition actor by rewriting the manifest, the only way a
/// repository names who may decide.
fn allow_actor(store: &StateStore, id: &str) {
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push(id.into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

/// Seed one contribution and one synthesis packet whose blocking evidence gap
/// keeps the not-yet-written proposal from requiring an assertion. The packet
/// owns the proposal target and adjudicates the proposal.
fn seed_blocked_evidence(store: &StateStore, scope: &ScopeId) {
    let contribution: provenance_core::Contribution = serde_json::from_value(json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default",
        "id": "contribution_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_overtime", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_overtime", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_overtime"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"},
        "open_questions": []
    }))
    .unwrap();
    let synthesis: provenance_core::SynthesisPacket = serde_json::from_value(json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default",
        "id": "synthesis_overtime",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [{"question": "Unverified", "needed_evidence_type": "source", "blocking_promotion": true}],
        "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_overtime", "proposal_key": "overtime",
            "proposal_type": "requirement_candidate", "summary": "Candidate",
            "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    }))
    .unwrap();
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

/// Clear the packet's blocking evidence gap once the proposal row exists, the
/// point where the run's adjudication has settled and the proposal may be
/// asserted.
fn qualify_seeded_evidence(store: &StateStore, scope: &ScopeId) {
    let mut packets = store.list_synthesis_packets(scope).unwrap();
    packets[0].evidence_gaps.clear();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::synthesis_packets_path(&store.layout, scope),
        &packets,
    )
    .unwrap();
}

fn supported_assertion(scope: &ScopeId) -> CreateAssertionInput {
    CreateAssertionInput {
        scope_id: scope.clone(),
        id: AssertionId::new("assertion_overtime").unwrap(),
        proposal_id: StableId::new("proposal_overtime").unwrap(),
        synthesis_packet_id: StableId::new("synthesis_overtime").unwrap(),
        supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
    }
}

fn write_failure(error: OperationError<WriteError>) -> WriteFailure {
    match error {
        OperationError::Handler(error) => error.safe(),
        OperationError::Common(_) => panic!("expected a handler refusal"),
    }
}

fn refusal_kind(error: OperationError<WriteError>) -> String {
    serde_json::to_value(write_failure(error)).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}
