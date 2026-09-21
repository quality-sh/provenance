//! Query pages hydrate only the records they serve.
//!
//! The match set behind a page can be arbitrarily wide. The reads behind
//! `neighbors`, `trace`, `evidence`, and `resolve_symbol` must choose the
//! page by id before any record decodes: the hydration count then stays
//! within the page plus fixed probes, an oversized record past the page
//! cannot refuse the page, and an oversized record inside the page still
//! refuses with its typed failure. Every count here reads the
//! `query_row_hydrated` probe, so a bound is proved, not inferred from a
//! final answer.

use std::cell::Cell;

use super::{root_of, seeded_store};
use crate::cache::tests::fixtures::{append_record, create_rule_of};
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::{
    read_failure::ReadFailure, Direction, EvidenceQuery, NeighborsQuery, ResolveSymbolQuery,
    TraceQuery, SDK_PROTOCOL_VERSION,
};
use provenance_core::{NodeType, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::verifies;
use serde_json::json;

/// Over the stored record ceiling, so the record can never decode.
const OVERSIZED_BYTES: usize = 70_000;

thread_local! {
    static HYDRATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Runs one query with the hydration probe armed, and reports how many
/// projection rows the run decoded.
async fn counted<R>(run: impl std::future::Future<Output = R>) -> (R, usize) {
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
fn append_requirement(
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
    record["refines"] = refines.map(json!);
    record["depends_on"] = json!(depends_on);
    record["supersedes"] = json!([]);
    append_record(&path, &record);
}

fn append_binding(
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

fn append_review(
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

fn neighbors_query(id: &str, limit: usize) -> NeighborsQuery {
    NeighborsQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        node_type: Some(NodeType::Requirement),
        direction: Direction::Both,
        relations: Vec::new(),
        limit,
    }
}

fn trace_query(id: &str, max_depth: usize, limit: usize) -> TraceQuery {
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

fn evidence_query(limit: usize) -> EvidenceQuery {
    EvidenceQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        rule: "rule_overtime".into(),
        base: None,
        head: None,
        limit,
    }
}

fn resolve_query(symbol: Option<&str>, limit: usize) -> ResolveSymbolQuery {
    ResolveSymbolQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        file: "src/pay.rs".into(),
        symbol: symbol.map(str::to_string),
        line: None,
        limit,
    }
}

fn neighbor_ids(result: &provenance_core::protocol::NeighborsResult) -> Vec<String> {
    result
        .neighbors
        .iter()
        .map(|neighbor| neighbor.node.id().as_str().to_string())
        .collect()
}

fn trace_ids(result: &provenance_core::protocol::TraceResult) -> Vec<(usize, String)> {
    result
        .nodes
        .iter()
        .map(|node| (node.depth, node.node.id().as_str().to_string()))
        .collect()
}

/// The seeded store's requirement already names its domain and carries
/// the seeded boundary, so a wide child set serves thirty children and
/// two fixed neighbours, children first in id order.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn neighbors_hydrate_only_the_served_page_over_a_wide_graph() {
    let (dir, store, scope) = seeded_store();
    for index in 0..30 {
        append_requirement(
            &store,
            &scope,
            &format!("req_child_{index:03}"),
            Some("req_overtime"),
            &[],
            0,
        );
    }
    let root = root_of(&dir);
    let policy = ReadPolicy::default();

    let (answer, _) = counted(queries::neighbors(
        Some(root.clone()),
        &scope,
        policy,
        neighbors_query("req_overtime", 200),
    ))
    .await;
    let full = answer.unwrap().result;
    let order = neighbor_ids(&full);
    assert_eq!(order.len(), 32, "thirty children and two fixed records");
    assert_eq!(order[30], "domain_payroll");
    assert_eq!(order[31], "boundary_no_backpay");
    let mut children = order[..30].to_vec();
    children.sort();
    assert_eq!(&order[..30], &children[..], "children serve in id order");
    assert!(!full.has_more);

    for limit in [1usize, 4, 29] {
        let (answer, hydrations) = counted(queries::neighbors(
            Some(root.clone()),
            &scope,
            policy,
            neighbors_query("req_overtime", limit),
        ))
        .await;
        let result = answer.unwrap().result;
        assert_eq!(neighbor_ids(&result), order[..limit]);
        assert!(result.has_more);
        assert!(
            hydrations <= limit + 2,
            "limit {limit} decoded {hydrations} rows for 32 matches"
        );
    }
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn an_oversized_neighbor_past_the_page_cannot_refuse_it() {
    let (dir, store, scope) = seeded_store();
    for index in 0..6 {
        append_requirement(
            &store,
            &scope,
            &format!("req_child_{index:03}"),
            Some("req_overtime"),
            &[],
            0,
        );
    }
    append_requirement(
        &store,
        &scope,
        "req_child_999",
        Some("req_overtime"),
        &[],
        OVERSIZED_BYTES,
    );
    let (answer, hydrations) = counted(queries::neighbors(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        neighbors_query("req_overtime", 5),
    ))
    .await;
    let result = answer.expect("the page answers past an oversized record").result;
    assert_eq!(
        neighbor_ids(&result),
        ["req_child_000", "req_child_001", "req_child_002", "req_child_003", "req_child_004"]
    );
    assert!(result.has_more, "the oversized record still counts");
    assert!(hydrations <= 7, "the oversized row must never decode");
}

#[tokio::test]
async fn an_oversized_neighbor_in_the_page_still_refuses() {
    let (dir, store, scope) = seeded_store();
    for (index, bytes) in [(0usize, 0usize), (1, OVERSIZED_BYTES), (2, 0)] {
        append_requirement(
            &store,
            &scope,
            &format!("req_child_{index:03}"),
            Some("req_overtime"),
            &[],
            bytes,
        );
    }
    let error = queries::neighbors(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        neighbors_query("req_overtime", 50),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageRecordTooLarge)
    );
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn trace_hydrates_only_a_bounded_breadth_page() {
    let (dir, store, scope) = seeded_store();
    for index in 0..30 {
        append_requirement(
            &store,
            &scope,
            &format!("req_child_{index:03}"),
            Some("req_overtime"),
            &[],
            0,
        );
    }
    let root = root_of(&dir);
    let policy = ReadPolicy::default();

    let (answer, _) = counted(queries::trace(
        Some(root.clone()),
        &scope,
        policy,
        trace_query("req_overtime", 1, 200),
    ))
    .await;
    let full = answer.unwrap().result;
    let order = trace_ids(&full);
    assert_eq!(order.len(), 32, "one breadth level holds every neighbour");
    assert!(order.iter().all(|(depth, _)| *depth == 1));
    assert!(!full.has_more);

    let (answer, hydrations) = counted(queries::trace(
        Some(root.clone()),
        &scope,
        policy,
        trace_query("req_overtime", 1, 1),
    ))
    .await;
    let result = answer.unwrap().result;
    assert_eq!(trace_ids(&result), order[..1]);
    assert!(result.has_more);
    assert!(hydrations <= 3, "one page row plus the origin probe");
}

/// Two bridges reach one shared island, so the duplicate paths must fold
/// into one served node, and the dangler's missing target must never
/// appear.
#[tokio::test]
async fn trace_folds_duplicate_paths_and_skips_missing_targets() {
    let (dir, store, scope) = seeded_store();
    for index in 0..3 {
        append_requirement(
            &store,
            &scope,
            &format!("req_bridge_{index:02}"),
            Some("req_overtime"),
            &["req_island_shared"],
            0,
        );
    }
    append_requirement(&store, &scope, "req_island_shared", None, &[], 0);
    append_requirement(
        &store,
        &scope,
        "req_dangler",
        Some("req_overtime"),
        &["req_missing"],
        0,
    );
    let answer = queries::trace(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        trace_query("req_overtime", 2, 200),
    )
    .await
    .unwrap()
    .result;
    let depth_one: Vec<String> = trace_ids(&answer)
        .into_iter()
        .filter(|(depth, _)| *depth == 1)
        .map(|(_, id)| id)
        .collect();
    assert_eq!(
        depth_one,
        ["req_bridge_00", "req_bridge_01", "req_bridge_02", "req_dangler"]
    );
    let depth_two: Vec<String> = trace_ids(&answer)
        .into_iter()
        .filter(|(depth, _)| *depth == 2)
        .map(|(_, id)| id)
        .collect();
    assert_eq!(depth_two, ["req_island_shared"], "one node per record");
    assert!(!answer.has_more);

    let answer = queries::trace(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        trace_query("req_overtime", 2, 4),
    )
    .await
    .unwrap()
    .result;
    assert_eq!(trace_ids(&answer).len(), 4, "the page stops at the limit");
    assert!(answer.has_more, "the depth two island still counts");
}

/// Ten bridges reach ten distinct islands, so an unbounded walk decodes
/// the whole second level only to cut it; the served page of four must
/// decode the four depth one rows alone.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn trace_hydrates_no_depth_beyond_a_full_page() {
    let (dir, store, scope) = seeded_store();
    for index in 0..10 {
        append_requirement(
            &store,
            &scope,
            &format!("req_wide_{index:02}"),
            Some("req_overtime"),
            &[&format!("req_island_{index:02}")],
            0,
        );
        append_requirement(&store, &scope, &format!("req_island_{index:02}"), None, &[], 0);
    }
    append_requirement(
        &store,
        &scope,
        "req_wide_99",
        Some("req_overtime"),
        &["req_missing"],
        0,
    );
    let root = root_of(&dir);
    let policy = ReadPolicy::default();

    let (answer, _) = counted(queries::trace(
        Some(root.clone()),
        &scope,
        policy,
        trace_query("req_overtime", 2, 200),
    ))
    .await;
    let full = answer.unwrap().result;
    let ids = trace_ids(&full);
    assert_eq!(ids.len(), 23, "eleven breadth one rows, ten islands");
    assert!(!ids.iter().any(|(_, id)| id == "req_missing"));
    assert!(!full.has_more);

    let (answer, hydrations) = counted(queries::trace(
        Some(root.clone()),
        &scope,
        policy,
        trace_query("req_overtime", 2, 4),
    ))
    .await;
    let result = answer.unwrap().result;
    assert_eq!(
        trace_ids(&result),
        [(1usize, "req_wide_00".into()), (1, "req_wide_01".into()), (1, "req_wide_02".into()), (1, "req_wide_03".into())]
    );
    assert!(result.has_more);
    assert!(hydrations <= 7, "a full page must not decode depth two");
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn evidence_hydrates_only_the_served_page() {
    let (dir, store, scope) = seeded_store();
    create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    for index in 0..30 {
        append_binding(
            &store,
            &scope,
            &format!("bind_{index:03}"),
            "rule_overtime",
            "pay",
            0,
        );
    }
    let (answer, hydrations) = counted(queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(3),
    ))
    .await;
    let result = answer.unwrap().result;
    let ids: Vec<String> = result
        .implementation_bindings
        .iter()
        .map(|binding| binding.id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["bind_000", "bind_001", "bind_002"]);
    assert!(result.implementation_bindings_has_more);
    assert!(result.has_more);
    assert!(hydrations <= 4, "three page rows for thirty matches");
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn an_oversized_binding_past_the_page_cannot_refuse_evidence() {
    let (dir, store, scope) = seeded_store();
    create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    append_binding(&store, &scope, "bind_000", "rule_overtime", "pay", 0);
    append_binding(&store, &scope, "bind_001", "rule_overtime", "pay", 0);
    append_binding(
        &store,
        &scope,
        "bind_999",
        "rule_overtime",
        "pay",
        OVERSIZED_BYTES,
    );
    let (answer, hydrations) = counted(queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(2),
    ))
    .await;
    let result = answer.expect("the page answers past an oversized record").result;
    let ids: Vec<String> = result
        .implementation_bindings
        .iter()
        .map(|binding| binding.id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["bind_000", "bind_001"]);
    assert!(result.implementation_bindings_has_more);
    assert!(hydrations <= 3, "the oversized row must never decode");
}

/// The open-only cut belongs to the candidate query: cleared reviews
/// between open ones neither serve nor consume a page slot.
#[tokio::test]
async fn evidence_pages_only_the_open_reviews() {
    let (dir, store, scope) = seeded_store();
    create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    for (id, cleared_at) in [
        ("review_a", None),
        ("review_b", Some(5)),
        ("review_c", None),
        ("review_d", Some(5)),
        ("review_e", None),
    ] {
        append_review(&store, &scope, id, cleared_at);
    }
    let answer = queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(2),
    )
    .await
    .unwrap()
    .result;
    let reviews: Vec<String> = answer
        .reviews
        .iter()
        .map(|review| review.id.as_str().to_string())
        .collect();
    assert_eq!(reviews, ["review_a", "review_c"]);
    assert!(answer.reviews_has_more);
    assert!(answer.review_required);
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn resolve_symbol_hydrates_only_the_rule_page() {
    let store = super::comparison::test_stores::seeded_queries();
    create_rule_of(&store.state_store(), &store.scope, "rule_overtime", "req_overtime");
    create_rule_of(&store.state_store(), &store.scope, "rule_audit", "req_overtime");
    create_rule_of(&store.state_store(), &store.scope, "rule_site_000", "req_overtime");
    for index in 0..30 {
        append_binding(
            &store.state_store(),
            &store.scope,
            &format!("bind_{index:03}"),
            &format!("rule_site_{index:03}"),
            "pay",
            0,
        );
    }
    append_binding(&store.state_store(), &store.scope, "bind_audit", "rule_audit", "audit", 0);

    let (answer, hydrations) = counted(queries::resolve_symbol(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        resolve_query(None, 2),
    ))
    .await;
    let result = answer.unwrap().result;
    let ids: Vec<String> = result
        .rules
        .iter()
        .map(|node| node.id().as_str().to_string())
        .collect();
    assert_eq!(ids, ["rule_audit", "rule_overtime"], "candidates sort by id");
    assert!(result.has_more);
    assert!(hydrations <= 3, "binding rows must never decode");

    let (answer, hydrations) = counted(queries::resolve_symbol(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        resolve_query(Some("pay"), 2),
    ))
    .await;
    let result = answer.unwrap().result;
    let ids: Vec<String> = result
        .rules
        .iter()
        .map(|node| node.id().as_str().to_string())
        .collect();
    assert_eq!(ids, ["rule_overtime", "rule_site_000"], "the symbol filters");
    assert!(result.has_more);
    assert!(hydrations <= 3, "binding rows must never decode");
}
