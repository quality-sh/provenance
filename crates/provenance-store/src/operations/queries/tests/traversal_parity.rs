//! Differential gate: the traversal core over the `RecordFront` must
//! reproduce the served executors byte for byte over the shared fixtures.

use crate::operations::queries::tests::originals::walk as legacy;
use crate::operations::traversal::{
    neighbors_raw as traversal_neighbors, trace_raw as traversal_trace, RecordFront,
    TraversalSource,
};
use provenance_core::protocol::take_page;

async fn neighbors_result<S: TraversalSource>(
    source: &S,
    request: NeighborsQuery,
) -> anyhow::Result<provenance_core::protocol::NeighborsResult> {
    let (neighbors, has_more) = take_page(
        traversal_neighbors(source, request.clone()).await?,
        request.limit,
    );
    Ok(provenance_core::protocol::NeighborsResult {
        stamp: None,
        id: request.id,
        limit: request.limit,
        has_more,
        neighbors,
        next_cursor: None,
    })
}

async fn trace_result<S: TraversalSource>(
    source: &S,
    request: TraceQuery,
) -> anyhow::Result<provenance_core::protocol::TraceResult> {
    let (nodes, has_more) = take_page(
        traversal_trace(source, request.clone()).await?,
        request.limit,
    );
    Ok(provenance_core::protocol::TraceResult {
        stamp: None,
        id: request.id,
        max_depth: request.max_depth,
        limit: request.limit,
        has_more,
        nodes,
        next_cursor: None,
    })
}
use crate::state_store::StateStore;
use provenance_core::protocol::{Direction, NeighborsQuery, TraceQuery};
use provenance_core::{EdgeType, NodeType, ScopeId, StableId};

fn seeded() -> (tempfile::TempDir, crate::layout::ProvenanceLayout, ScopeId) {
    let (dir, layout, scope) = crate::cache::tests::fixtures::seeded_layout();
    // Grow the fixture so walks have breadth: requirements, edges, rules.
    let store = StateStore::new(layout.clone());
    crate::cache::tests::fixtures::create_requirement(
        &store,
        &scope,
        "req_breadth_one",
        provenance_core::RequirementStatus::Active,
    );
    crate::cache::tests::fixtures::create_requirement(
        &store,
        &scope,
        "req_breadth_two",
        provenance_core::RequirementStatus::Active,
    );
    crate::cache::tests::fixtures::create_requirement(
        &store,
        &scope,
        "req_breadth_three",
        provenance_core::RequirementStatus::Active,
    );
    for requirement in ["req_breadth_one", "req_breadth_two", "req_breadth_three"] {
        store
            .add_source_reference(crate::state_store::AddSourceReferenceInput {
                scope_id: scope.clone(),
                source_id: StableId::new("source_schads").unwrap(),
                requirement_id: StableId::new(requirement).unwrap(),
                clause: None,
            })
            .unwrap();
    }
    (dir, layout, scope)
}

fn neighbors_query(id: &str) -> NeighborsQuery {
    NeighborsQuery {
        protocol_version: None,
        id: id.to_string(),
        node_type: None,
        direction: Direction::Both,
        edge_types: vec![],
        include_retired: false,
        limit: 50,
        cursor: None,
    }
}

fn trace_query(id: &str) -> TraceQuery {
    TraceQuery {
        protocol_version: None,
        id: id.to_string(),
        node_type: None,
        direction: Direction::Both,
        edge_types: vec![],
        max_depth: 3,
        include_retired: false,
        limit: 50,
        cursor: None,
        visit_budget: None,
    }
}

#[tokio::test]
async fn neighbors_over_the_record_front_match_the_served_executor() {
    let (_dir, layout, scope) = seeded();
    let store = StateStore::new(layout.clone());
    let legacy_result =
        legacy::neighbors(&store, &scope, neighbors_query("source_schads")).unwrap();
    let source = RecordFront::load(&store, &scope, false).unwrap();

    let served = neighbors_result(&source, neighbors_query("source_schads"))
        .await
        .unwrap();

    assert_eq!(
        serde_json::to_string(&legacy_result).unwrap(),
        serde_json::to_string(&served).unwrap()
    );
}

#[tokio::test]
async fn trace_over_the_record_front_matches_the_served_executor() {
    let (_dir, layout, scope) = seeded();
    let store = StateStore::new(layout.clone());
    let legacy_result = legacy::trace(&store, &scope, trace_query("source_schads")).unwrap();
    let source = RecordFront::load(&store, &scope, false).unwrap();

    let served = trace_result(&source, trace_query("source_schads"))
        .await
        .unwrap();

    assert_eq!(
        serde_json::to_string(&legacy_result).unwrap(),
        serde_json::to_string(&served).unwrap()
    );
}

#[tokio::test]
async fn traversal_ordering_contract_is_rank_then_id() {
    let (_dir, layout, scope) = seeded();
    let store = StateStore::new(layout.clone());
    let source = RecordFront::load(&store, &scope, false).unwrap();

    let result = neighbors_result(&source, neighbors_query("source_schads"))
        .await
        .unwrap();

    let ids: Vec<String> = result
        .neighbors
        .iter()
        .map(|neighbor| neighbor.node.id().as_str().to_string())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "neighbors order by canonical id within rank");
}

#[test]
fn record_front_reports_node_kinds_it_holds() {
    let (_dir, layout, scope) = seeded();
    let store = StateStore::new(layout);
    let source = RecordFront::load(&store, &scope, false).unwrap();

    assert!(source
        .find_blocking(
            NodeType::Source,
            &StableId::new("source_schads").unwrap(),
            false
        )
        .unwrap()
        .is_some());
    assert!(source
        .find_blocking(
            NodeType::Source,
            &StableId::new("source_absent").unwrap(),
            false
        )
        .unwrap()
        .is_none());
    let _ = EdgeType::References;
}
