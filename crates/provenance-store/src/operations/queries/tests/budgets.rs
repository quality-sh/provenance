use super::{root_of, seeded_store};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::{queries, read_policy::ReadPolicy};
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::{
    read_failure::ReadFailure, Direction, EvidenceQuery, NeighborsQuery, QueryResponse,
    SDK_PROTOCOL_VERSION,
};
use provenance_core::NodeType;
use provenance_macros::verifies;
use serde_json::json;
use std::sync::Arc;

const LARGE_FIELD_BYTES: usize = 60_000;

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
            scope: provenance_core::ScopeId::new("default").unwrap(),
            policy: ReadPolicy::default(),
            requested_target: "test".into(),
            external: true,
        }))
    }
}

fn append_large_children(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    count: usize,
) {
    let path = crate::shards::requirements_path(&store.layout, scope);
    let mut record = json!(store.list_requirements(scope).unwrap()[0]);
    record["description"] = json!("x".repeat(LARGE_FIELD_BYTES));
    record["refines"] = json!("req_overtime");
    for index in 0..count {
        record["id"] = json!(format!("req_large_{index:03}"));
        crate::cache::tests::fixtures::append_record(&path, &record);
    }
}

fn neighbors_query() -> NeighborsQuery {
    NeighborsQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: "req_overtime".into(),
        node_type: Some(NodeType::Requirement),
        direction: Direction::Both,
        relations: Vec::new(),
        limit: 200,
    }
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn neighbors_keeps_an_under_budget_response_and_its_wire_envelope() {
    let (dir, store, scope) = seeded_store();
    append_large_children(&store, &scope, 1);

    let answer = queries::neighbors(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        neighbors_query(),
    )
    .await
    .unwrap();
    assert_eq!(answer.result.neighbors.len(), 1);
    assert_eq!(answer.result.neighbors[0].node.id().as_str(), "req_large_000");
    let wire = serde_json::to_vec(&QueryResponse::new("neighbors", answer)).unwrap();
    assert!(wire.len() <= super::super::page::RESPONSE_BYTES);
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn neighbors_refuses_an_oversized_response_through_native_and_registered_calls() {
    let (dir, store, scope) = seeded_store();
    append_large_children(&store, &scope, 20);
    let root = root_of(&dir);

    let error = queries::neighbors(
        Some(root.clone()),
        &scope,
        ReadPolicy::default(),
        neighbors_query(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded)
    );

    let failure = catalog::invoke_with(
        "neighbors",
        SDK_PROTOCOL_VERSION,
        json!({
            "context": {"repository": "test", "scope": "default"},
            "request": {
                "id": "req_overtime",
                "node_type": "requirement",
                "limit": 200
            }
        }),
        Arc::new(Target(root)),
    )
    .await
    .unwrap_err();
    assert_eq!(failure.status_code(), 409);
    assert_eq!(failure.error, json!({"kind": "page_budget_exceeded"}));
}

fn append_verification_binding(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    declared_by: String,
) {
    let path = crate::shards::verification_bindings_path(&store.layout, scope);
    crate::cache::tests::fixtures::append_record(
        &path,
        &json!({
            "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION,
            "scope_id": scope.as_str(),
            "id": "verification_large",
            "rule_id": "rule_overtime",
            "key": "large",
            "method": "examples",
            "declared_by": declared_by,
            "file": "src/pay.rs",
            "symbol": "pay"
        }),
    );
}

fn evidence_query() -> EvidenceQuery {
    EvidenceQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        rule: "rule_overtime".into(),
        base: None,
        head: None,
        limit: 200,
    }
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn evidence_keeps_an_under_budget_record() {
    let (dir, store, scope) = seeded_store();
    crate::cache::tests::fixtures::create_rule_of(
        &store,
        &scope,
        "rule_overtime",
        "req_overtime",
    );
    append_verification_binding(&store, &scope, "x".repeat(LARGE_FIELD_BYTES));

    let answer = queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(),
    )
    .await
    .unwrap();
    assert_eq!(answer.result.verification_bindings.len(), 1);
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn evidence_refuses_one_record_over_the_record_budget() {
    let (dir, store, scope) = seeded_store();
    crate::cache::tests::fixtures::create_rule_of(
        &store,
        &scope,
        "rule_overtime",
        "req_overtime",
    );
    append_verification_binding(&store, &scope, "x".repeat(70_000));

    let error = queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageRecordTooLarge)
    );
}
