//! Reproduction: a Source write the transport accepts can exceed the
//! supported resource read budget, so the accepted record is unreadable.

use super::initialized_store;
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::state_store::CreateSourceInput;
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::SDK_PROTOCOL_VERSION;
use provenance_core::{SourceType, StableId, ScopeId};
use serde_json::json;
use std::sync::Arc;

/// The read budget the supported resource member and page reads enforce.
const READ_BUDGET: usize = crate::cache::read::page::RESOURCE_RECORD_BYTES;

/// The transport body budget the HTTP and MCP surfaces accept.
const TRANSPORT_BUDGET: usize = 1024 * 1024;

struct Target(camino::Utf8PathBuf);

impl ContextResolver for Target {
    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::read(PreparedRead {
            root: self.0.clone(),
            scope: ScopeId::new("default").unwrap(),
            policy: Default::default(),
            requested_target: "test".into(),
            external: true,
        }))
    }
}

fn oversized_input(scope: &ScopeId, name_bytes: usize) -> CreateSourceInput {
    CreateSourceInput {
        scope_id: scope.clone(),
        id: StableId::new("source_oversized").unwrap(),
        name: "x".repeat(name_bytes),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

/// The whole request body stays inside the transport budget while the
/// stored record exceeds the read budget.
#[test]
fn reproduction_request_fits_transport_but_exceeds_read_budget() {
    let (dir, store, scope) = initialized_store();
    let input = oversized_input(&scope, READ_BUDGET);
    let body = serde_json::to_vec(&input).unwrap().len();
    assert!(
        body < TRANSPORT_BUDGET,
        "fixture body {body} must stay inside the transport budget"
    );
    let outcome = store.create_source(input);
    drop(store);
    drop(dir);
    let published = outcome.is_ok();
    let _ = published;
}

#[tokio::test]
async fn reproduction_accepted_oversized_source_fails_member_read() {
    let (dir, store, scope) = initialized_store();
    store
        .create_source(oversized_input(&scope, READ_BUDGET))
        .expect("current behaviour: the oversized write is accepted");
    let root = dir.path().to_path_buf();
    let root = camino::Utf8PathBuf::from_path_buf(root).unwrap();
    let failure = catalog::invoke_with(
        "get-source-v2",
        catalog::SDK_PROTOCOL_VERSION,
        json!({
            "context": {"repository": "test", "scope": "default"},
            "request": {"id": "source_oversized"}
        }),
        Arc::new(Target(root.clone())),
    )
    .await
    .expect_err("current behaviour: the member read refuses the accepted record");
    assert!(
        format!("{failure}").contains("page_record_too_large")
            || format!("{failure:?}").contains("PageRecordTooLarge"),
        "unexpected failure: {failure:?}"
    );
    let list = catalog::invoke_with(
        "page-sources-v2",
        catalog::SDK_PROTOCOL_VERSION,
        json!({
            "context": {"repository": "test", "scope": "default"},
            "request": {"limit": 50, "cursor": null}
        }),
        Arc::new(Target(root)),
    )
    .await;
    assert!(
        list.is_err(),
        "current behaviour: the list read refuses the accepted record"
    );
}
