use super::super::{
    invoke_with, ContextResolver, ExecutionNeeds, PreparedContext, PreparedScope, RequestedContext,
};
use crate::layout::ProvenanceLayout;
use provenance_core::{
    protocol::failure::OperationFailure, Manifest, RepoPathPrefix, Scope, ScopeId,
    SDK_PROTOCOL_VERSION,
};
use serde_json::{json, Value};
use std::sync::Arc;

struct Resolver(PreparedScope);

impl ContextResolver for Resolver {
    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::for_scope(self.0.clone()))
    }
}

#[tokio::test]
async fn discussion_write_scope_mismatch_keeps_its_safe_status() {
    let directory = tempfile::tempdir().unwrap();
    let error = invoke_with(
        "write-discussion-v2",
        SDK_PROTOCOL_VERSION,
        json!({
            "context": {"repository": "fixture", "scope": "selected"},
            "request": {
                "scope_id": "other",
                "parent": {"node_type": "requirement", "node_id": "req_one"},
                "request_id": "request_one",
                "actor": "ben",
                "declared_by": null,
                "action": {"kind": "start", "role": "user", "body": "Concern"}
            }
        }),
        Arc::new(Resolver(PreparedScope {
            root: camino::Utf8PathBuf::from_path_buf(directory.path().into()).unwrap(),
            scope: ScopeId::new("selected").unwrap(),
            requested_target: "fixture".into(),
        })),
    )
    .await
    .unwrap_err();

    assert_eq!(error.error, json!({"kind": "scope_mismatch"}));
    assert_eq!(error.status_code(), 400);
}

fn review_fixture() -> (tempfile::TempDir, ProvenanceLayout, Arc<Resolver>) {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().into()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let selected = ScopeId::new("selected").unwrap();
    let mut manifest = Manifest::default_with_scope(selected.clone(), RepoPathPrefix::new("."));
    manifest.scopes.push(Scope {
        id: ScopeId::new("other").unwrap(),
        path_prefix: RepoPathPrefix::new("other"),
    });
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let resolver = Arc::new(Resolver(PreparedScope {
        root,
        scope: selected,
        requested_target: "fixture".into(),
    }));
    (directory, layout, resolver)
}

async fn assert_review_scope_mismatch(operation: &str, request: Value) {
    let (_directory, layout, resolver) = review_fixture();
    let error = invoke_with(
        operation,
        SDK_PROTOCOL_VERSION,
        json!({
            "context": {"repository": "fixture", "scope": "selected"},
            "request": request,
        }),
        resolver,
    )
    .await
    .unwrap_err();

    assert_eq!(error.error, json!({"kind": "scope_mismatch"}));
    assert_eq!(error.status_code(), 400);
    assert!(!layout.scopes_dir().exists());
}

#[tokio::test]
async fn submit_requirement_review_rejects_a_scope_mismatch_without_writing() {
    assert_review_scope_mismatch(
        "submit-requirement-review-v2",
        json!({
            "scope_id": "other",
            "request_id": "request_submit",
            "actor": "agent",
            "requirement_id": "req_one",
            "declared_by": null,
            "proposal_id": "proposal_one",
            "proposal_key": "proposal-key",
            "title": "Review title",
            "summary": "Review summary",
            "confidence": null,
            "source_ids": [],
            "evidence_references": [],
            "builds_on": [],
            "expected_revision": null,
            "revises": null
        }),
    )
    .await;
}

#[tokio::test]
async fn decide_requirement_review_rejects_a_scope_mismatch_without_writing() {
    assert_review_scope_mismatch(
        "decide-requirement-review-v2",
        json!({
            "scope_id": "other",
            "requirement_id": "req_one",
            "request_id": "request_decide",
            "actor": {"identity_type": "human", "id": "reviewer"},
            "proposal_id": "proposal_one",
            "disposition_id": "disposition_one",
            "decision": "rejected",
            "rationale": "The revision needs work.",
            "canonical_artifact": null,
            "feedback": null,
            "declared_by": null
        }),
    )
    .await;
}

#[tokio::test]
async fn withdraw_requirement_review_rejects_a_scope_mismatch_without_writing() {
    assert_review_scope_mismatch(
        "withdraw-requirement-review-v2",
        json!({
            "scope_id": "other",
            "requirement_id": "req_one",
            "request_id": "request_withdraw",
            "actor": "agent",
            "proposal_id": "proposal_one",
            "declared_by": null,
            "reason": "The author will revise the proposal."
        }),
    )
    .await;
}
