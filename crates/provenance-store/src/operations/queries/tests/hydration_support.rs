//! Shared setup for the page-bound tests: the hydration probe, raw
//! record writers past the writers' checks, and the query builders.

use std::cell::Cell;

use crate::cache::tests::fixtures::append_record;
use provenance_core::protocol::{
    Direction, EvidenceQuery, NeighborsQuery, ResolveSymbolQuery, TraceQuery, SDK_PROTOCOL_VERSION,
};
use provenance_core::{NodeType, SUPPORTED_SCHEMA_VERSION};
use serde_json::json;

/// Over the stored record ceiling, so the record can never decode.
pub const OVERSIZED_BYTES: usize = 70_000;

thread_local! {
    static HYDRATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Runs one query with the hydration probe armed, and reports how many
/// projection rows the run decoded.
pub async fn counted<R>(run: impl std::future::Future<Output = R>) -> (R, usize) {
    HYDRATIONS.with(|count| count.set(0));
    crate::test_probes::arm("query_row_hydrated", || {
        HYDRATIONS.with(|count| count.set(count.get() + 1));
        Ok(())
    });
    let answer = run.await;
    crate::test_probes::disarm("query_row_hydrated");
    let hydrations = HYDRATIONS.with(|count| count.get());
    (answer, hydrations)
}

/// One raw requirement beside `req_overtime`, written past the writers'
/// checks so the test controls every relation field. The domain pointer
/// is dropped, so the graph holds exactly what the test names.
pub fn append_requirement(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    id: &str,
    refines: Option<&str>,
    depends_on: &[&str],
    description_bytes: usize,
) {
    let path = crate::shards::requirements_path(&store.layout, scope);
    let mut record = json!(store.list_requirements(scope).unwrap()[0]);
    record["id"] = json!(id);
    record["description"] = json!("x".repeat(description_bytes));
    record["domain_id"] = json!(None::<String>);
    record["refines"] = refines.map(|value| json!(value)).unwrap_or(json!(null));
    record["depends_on"] = json!(depends_on);
    record["supersedes"] = json!([]);
    append_record(&path, &record);
}

pub fn append_binding(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    id: &str,
    rule_id: &str,
    symbol: &str,
    declared_by_bytes: usize,
) {
    append_record(
        &crate::shards::implementation_bindings_path(&store.layout, scope),
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": scope.as_str(),
            "id": id,
            "rule_id": rule_id,
            "declared_by": "x".repeat(declared_by_bytes),
            "file": "src/pay.rs",
            "symbol": symbol,
        }),
    );
}

pub fn append_review(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    id: &str,
    cleared_at: Option<i64>,
) {
    let mut record = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": scope.as_str(),
        "id": id,
        "rule_id": "rule_overtime",
        "requirement_id": "req_overtime",
        "field": "statement",
        "before": "before",
        "after": "after",
        "changed_at": 1,
    });
    if let Some(cleared_at) = cleared_at {
        record["cleared_at"] = json!(cleared_at);
        record["cleared_by_run"] = json!("run_cleared");
    }
    append_record(
        &crate::shards::requirement_reviews_path(&store.layout, scope),
        &record,
    );
}

pub fn neighbors_query(id: &str, limit: usize) -> NeighborsQuery {
    NeighborsQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        node_type: Some(NodeType::Requirement),
        direction: Direction::Both,
        relations: Vec::new(),
        limit,
    }
}

pub fn trace_query(id: &str, max_depth: usize, limit: usize) -> TraceQuery {
    TraceQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        node_type: Some(NodeType::Requirement),
        direction: Direction::Both,
        relations: Vec::new(),
        max_depth,
        limit,
    }
}

pub fn evidence_query(limit: usize) -> EvidenceQuery {
    EvidenceQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        rule: "rule_overtime".into(),
        base: None,
        head: None,
        limit,
    }
}

pub fn resolve_query(symbol: Option<&str>, limit: usize) -> ResolveSymbolQuery {
    ResolveSymbolQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        file: "src/pay.rs".into(),
        symbol: symbol.map(str::to_string),
        line: None,
        limit,
    }
}
