//! Fixture and helpers of the native review submission tests.

use provenance_core::{
    CanonicalArtifact, CanonicalArtifactType, DispositionActor, DispositionDecision, IdentityType,
    NodeType, ScopeId, StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    review::{DecideRecordReview, SubmitRecordReview},
    state_store::{StateStore, TypedSpecInput, UpdateRuleInput},
};
use serde_json::{json, Value};

pub fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}"#,
    )
    .unwrap();
    let store = provenance_store::repository::Repository::open(root, "default")
        .unwrap()
        .store();
    (temp, store, ScopeId::new("default").unwrap())
}

pub fn input<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

pub fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// The review submissions stored for one record.
pub fn submissions(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) -> usize {
    let prefix = format!("review:{}:{record}:", kind.as_str());
    store
        .list_proposal_definitions(scope)
        .unwrap()
        .iter()
        .filter(|proposal| proposal.proposal_key.starts_with(&prefix))
        .count()
}

/// The pending submission is bound to the current review revision.
pub fn assert_pending_at_current(
    store: &StateStore,
    scope: &ScopeId,
    kind: NodeType,
    record: &str,
) {
    let state = store
        .record_decision_state(scope, kind, &id(record))
        .unwrap();
    let pending = state
        .pending
        .unwrap_or_else(|| panic!("{record}: no pending submission"));
    assert_eq!(Some(pending.revision), state.current_revision, "{record}");
}

pub fn requirement(store: &StateStore, record: &str, parents: &[&str]) {
    store
        .create_requirement(input(json!({
            "scope_id":"default", "id":record, "statement":"The system stores records.",
            "status":"active", "refines":parents, "depends_on":[], "supersedes":[]
        })))
        .unwrap();
}

pub fn rule(store: &StateStore, record: &str, status: &str, parents: &[&str]) {
    store
        .create_rule(input(json!({
            "scope_id":"default", "id":record, "statement":"The system keeps each record.",
            "status":status, "severity":"medium", "requirement_ids":parents,
            "resolution_ids":[]
        })))
        .unwrap();
}

pub fn resolution(store: &StateStore, record: &str, status: &str, parents: &[&str]) {
    store
        .create_resolution(input(json!({
            "scope_id":"default", "id":record, "title":"Storage",
            "position":"Store records in one file.", "rationale":"One file is simple.",
            "status":status, "requirement_ids":parents, "supersedes":[], "inputs":[]
        })))
        .unwrap();
}

/// Accepts the pending submission of the current revision. Opens it first
/// when the record has none.
pub fn accept_current(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) {
    let state = store
        .record_decision_state(scope, kind, &id(record))
        .unwrap();
    let proposal = match state.pending {
        Some(pending) => pending.proposal_id,
        None => {
            store
                .submit_record_review(SubmitRecordReview {
                    scope_id: scope.clone(),
                    actor: "author".into(),
                    record_kind: kind,
                    record_id: id(record),
                    declared_by: None,
                    title: "Review the record".into(),
                    summary: "Review the stored text.".into(),
                    confidence: None,
                    source_ids: Vec::new(),
                    evidence_references: Vec::new(),
                    builds_on: Vec::new(),
                    expected_revision: None,
                    revises: None,
                })
                .unwrap()
                .proposal_id
        }
    };
    store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: DispositionActor {
                identity_type: IdentityType::Human,
                id: "reviewer".into(),
                name: None,
            },
            proposal_id: proposal,
            decision: DispositionDecision::Accepted,
            rationale: None,
            canonical_artifact: Some(CanonicalArtifact {
                artifact_type: input::<CanonicalArtifactType>(json!(kind.as_str())),
                artifact_id: id(record),
            }),
            feedback: None,
            declared_by: None,
        })
        .unwrap();
}

pub fn update_rule(store: &StateStore, change: &Value) -> anyhow::Result<provenance_core::Rule> {
    let mut request = json!({"scope_id":"default", "id":"rule_a"});
    request
        .as_object_mut()
        .unwrap()
        .extend(change.as_object().unwrap().clone());
    store.update_rule(input::<UpdateRuleInput>(request))
}

pub fn etag(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) -> String {
    store
        .record_edit_state(scope, kind, &id(record))
        .unwrap()
        .etag
}

pub fn status(store: &StateStore, scope: &ScopeId, kind: NodeType, record: &str) -> Value {
    let value = match kind {
        NodeType::Requirement => serde_json::to_value(store.list_requirements(scope).unwrap()),
        NodeType::Rule => serde_json::to_value(store.list_rules(scope).unwrap()),
        _ => serde_json::to_value(store.list_resolutions(scope).unwrap()),
    }
    .unwrap();
    value
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == record)
        .unwrap()["status"]
        .clone()
}

pub fn typed_rule() -> TypedSpecInput {
    input(json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "spec":"storage", "declared_by":"spec://storage", "adopt_unowned":[],
        "sources":[{"key":"policy","id":"source_a","name":"Policy","kind":"document"}],
        "requirements":[{"key":"storage","id":"req_a","statement":"The system stores records.",
            "sources":["policy"]}],
        "rules":[{"key":"keep","id":"rule_a","requirements":["storage"],
            "statement":"The system keeps each record."}]
    }))
}
