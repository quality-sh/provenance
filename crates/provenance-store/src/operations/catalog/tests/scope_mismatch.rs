use super::super::{
    invoke_with, ContextResolver, ExecutionNeeds, PreparedContext, PreparedScope, RequestedContext,
};
use provenance_core::{protocol::failure::OperationFailure, ScopeId, SDK_PROTOCOL_VERSION};
use serde_json::json;
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
