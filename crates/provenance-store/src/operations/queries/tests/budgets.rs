use super::{root_of, seeded_store};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::{
    queries,
    read_policy::{FreshnessPolicy, ReadPolicy},
};
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::{
    read_failure::ReadFailure, Direction, EvidenceQuery, GetQuery, NeighborsQuery, QueryResponse,
    Stamp, StampPolicy, Stamped, SDK_PROTOCOL_VERSION,
};
use provenance_core::model::ProjectionRow;
use provenance_core::{NodeType, Rule};
use provenance_macros::verifies;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;

const LARGE_FIELD_BYTES: usize = 60_000;

#[derive(Clone, Debug, Serialize)]
struct Payload {
    payload: String,
}

fn stamped_payload(payload: String) -> Stamped<Payload> {
    Stamped {
        result: Payload { payload },
        stamp: Stamp {
            serial: 1,
            digest: "digest".into(),
            instance_id: "instance".into(),
            derivation: 1,
            policy: StampPolicy::AnnotateOnly,
            attested: vec!["requirements".into()],
            live: Vec::new(),
        },
        freshness_error: None,
    }
}

#[test]
fn shared_response_check_counts_the_exact_serialized_query_envelope() {
    let empty = serde_json::to_vec(&QueryResponse::new("test", stamped_payload(String::new())))
        .unwrap()
        .len();
    let remaining = provenance_core::protocol::QUERY_RESPONSE_BYTES - empty;
    let at_limit = stamped_payload("x".repeat(remaining));
    assert_eq!(
        serde_json::to_vec(&QueryResponse::new("test", at_limit.clone()))
            .unwrap()
            .len(),
        provenance_core::protocol::QUERY_RESPONSE_BYTES
    );
    super::super::page::checked("test", at_limit).unwrap();

    let error = super::super::page::checked("test", stamped_payload("x".repeat(remaining + 1)))
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded)
    );
}

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
    assert!(answer
        .result
        .neighbors
        .iter()
        .any(|neighbor| neighbor.node.id().as_str() == "req_large_000"));
    let wire = serde_json::to_vec(&QueryResponse::new("neighbors", answer)).unwrap();
    assert!(wire.len() <= provenance_core::protocol::QUERY_RESPONSE_BYTES);
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
    declared_by: &str,
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
    crate::cache::tests::fixtures::create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    append_verification_binding(&store, &scope, &"x".repeat(LARGE_FIELD_BYTES));

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
    crate::cache::tests::fixtures::create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    append_verification_binding(&store, &scope, &"x".repeat(70_000));

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

async fn set_rule_description_bytes(
    store: &crate::state_store::StateStore,
    id: &str,
    description_bytes: usize,
) -> usize {
    let cache = crate::cache::open_cache(&store.layout).await.unwrap();
    sqlx::query("UPDATE rules SET description = ? WHERE id = ?")
        .bind("x".repeat(description_bytes))
        .bind(id)
        .execute(cache.pool())
        .await
        .unwrap();
    let expression = crate::cache::read::page::byte_expression(Rule::COLUMNS);
    let size: i64 = sqlx::query_scalar(&format!(
        "SELECT {expression} FROM rules WHERE id = ?"
    ))
    .bind(id)
    .fetch_one(cache.pool())
    .await
    .unwrap();
    cache.close().await.unwrap();
    usize::try_from(size).unwrap()
}

fn get_rule_query() -> GetQuery {
    GetQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        node_type: NodeType::Rule,
        id: "rule_overtime".into(),
    }
}

async fn get_rule_member(root: camino::Utf8PathBuf) -> serde_json::Value {
    catalog::invoke_with(
        "get-rule-v2",
        SDK_PROTOCOL_VERSION,
        json!({
            "context": {"repository": "test", "scope": "default"},
            "request": {"id": "rule_overtime"}
        }),
        Arc::new(Target(root)),
    )
    .await
    .unwrap_or_else(|failure| {
        panic!(
            "member read failed with {}: {}",
            failure.status_code(),
            failure.error
        )
    })
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn stored_record_bytes_decide_query_eligibility_not_wrapper_bytes() {
    let (dir, store, scope) = seeded_store();
    crate::cache::tests::fixtures::create_rule_of(
        &store,
        &scope,
        "rule_overtime",
        "req_overtime",
    );
    let root = root_of(&dir);
    queries::get(
        Some(root.clone()),
        &scope,
        ReadPolicy::default(),
        get_rule_query(),
    )
    .await
    .unwrap();

    let base = set_rule_description_bytes(&store, "rule_overtime", 0).await;
    let description_bytes = crate::operations::reader::RECORD_BYTES - base;
    let size = set_rule_description_bytes(&store, "rule_overtime", description_bytes).await;
    assert_eq!(size, crate::operations::reader::RECORD_BYTES);

    let answer = queries::get(
        Some(root.clone()),
        &scope,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
        get_rule_query(),
    )
    .await
    .unwrap();
    let node = answer.result.node.as_ref().unwrap();
    assert!(
        serde_json::to_vec(node).unwrap().len() > crate::operations::reader::RECORD_BYTES,
        "the traversal wrapper must be larger than its stored row"
    );
    assert_eq!(
        get_rule_member(root.clone()).await["result"]["id"],
        "rule_overtime"
    );

    let size = set_rule_description_bytes(&store, "rule_overtime", description_bytes + 1).await;
    assert_eq!(size, crate::operations::reader::RECORD_BYTES + 1);
    let error = queries::get(
        Some(root.clone()),
        &scope,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
        get_rule_query(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageRecordTooLarge)
    );
    assert_eq!(
        get_rule_member(root).await["result"]["id"],
        "rule_overtime"
    );
}
