#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
}
use axum::{
    body::Body,
    http::{HeaderMap, Request},
};
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    HostAccess, ReviewPageWrite, StatementHost,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::records::Repository;
use tower::ServiceExt as _;

// Implementation aid: the access policy makes an unrelated scope unreadable after
// capture starts. The selected-scope write can commit before the final snapshot fails.
struct UnreadableScope {
    access: FixtureAccess,
    shard: std::path::PathBuf,
}
impl ContextResolver for UnreadableScope {
    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        let prepared = self.access.prepare(operation, context, needs)?;
        std::fs::write(&self.shard, "invalid canonical JSON\n").unwrap();
        Ok(prepared)
    }
}
impl HostAccess for UnreadableScope {
    fn authenticate(&self, headers: &HeaderMap) -> Result<(), OperationFailure> {
        self.access.authenticate(headers)
    }
    fn advertises(&self, operation: &str) -> bool {
        self.access.advertises(operation)
    }
    fn bound_identity(&self) -> Option<(String, String)> {
        self.access.bound_identity()
    }
}

#[tokio::test]
async fn a_saved_page_write_succeeds_when_the_notification_snapshot_fails() {
    let repo = Repository::new("The graph is readable.");
    repo.add_scope("other", "The other graph is readable.");
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let seen = notifications.clone();
    let host = StatementHost::with_access(Arc::new(UnreadableScope {
        access,
        shard: repo
            .dir
            .path()
            .join(".provenance/state/scopes/other/rules/rule.jsonl"),
    }))
    .with_page_writes(
        repo.layout.clone(),
        Arc::new(move |changes| seen.lock().unwrap().push(changes)),
    );
    let mut request = Request::builder()
        .method("POST")
        .uri("/rules")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"data":{
                "id":"rule_saved", "statement":"The saved write succeeds.", "status":"draft",
                "severity":"high", "requirement_ids":["req_shared"], "resolution_ids":[]
            }})
            .to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(ReviewPageWrite);
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(std::fs::read_to_string(
        repo.dir
            .path()
            .join(".provenance/state/scopes/default/rules/rule.jsonl")
    )
    .unwrap()
    .contains("rule_saved"));
    assert_eq!(status.as_u16(), 200, "{}", String::from_utf8_lossy(&body));
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["data"]["id"], "rule_saved");
    let notifications = notifications.lock().unwrap();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].configuration_changed);
    assert!(notifications[0].scopes.is_empty());
}
