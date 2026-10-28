//! Walk pages hydrate only the records they serve.
//!
//! The match set behind a `neighbors` or `trace` page can be arbitrarily
//! wide. The walk must choose the page by id before any record decodes:
//! the hydration count then stays within the page plus fixed probes, an
//! oversized record past the page cannot refuse the page, and an
//! oversized record inside the page still refuses with its typed
//! failure. Every count here reads the `query_row_hydrated` probe, so a
//! bound is proved, not inferred from a final answer.

use super::hydration_support::{append_requirement, counted, neighbors_query, trace_query};
use super::{root_of, seeded_store};
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::{read_failure::ReadFailure, NeighborsResult, TraceResult};
use provenance_macros::verifies;

fn neighbor_ids(result: &NeighborsResult) -> Vec<String> {
    result
        .neighbors
        .iter()
        .map(|neighbor| neighbor.node.id().as_str().to_string())
        .collect()
}

fn trace_ids(result: &TraceResult) -> Vec<(usize, String)> {
    result
        .nodes
        .iter()
        .map(|node| (node.depth, node.node.id().as_str().to_string()))
        .collect()
}

/// Thirty children refine the seeded requirement, which also names its
/// domain and carries the seeded boundary: thirty-two neighbours, the
/// children first in id order.
fn wide_children(store: &crate::state_store::StateStore, scope: &provenance_core::ScopeId) {
    for index in 0..30 {
        append_requirement(
            store,
            scope,
            &format!("req_child_{index:03}"),
            Some("req_overtime"),
            &[],
            0,
        );
    }
}

/// The seeded store's requirement already names its domain and carries
/// the seeded boundary, so a wide child set serves thirty children and
/// two fixed neighbours, children first in id order.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn neighbors_hydrate_only_the_served_page_over_a_wide_graph() {
    let (dir, store, scope) = seeded_store();
    wide_children(&store, &scope);
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
        assert_eq!(neighbor_ids(&result), order[..limit].to_vec());
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
        super::hydration_support::OVERSIZED_BYTES,
    );
    let (answer, hydrations) = counted(queries::neighbors(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        neighbors_query("req_overtime", 5),
    ))
    .await;
    let result = answer
        .expect("the page answers past an oversized record")
        .result;
    assert_eq!(
        neighbor_ids(&result),
        [
            "req_child_000",
            "req_child_001",
            "req_child_002",
            "req_child_003",
            "req_child_004"
        ]
    );
    assert!(result.has_more, "the oversized record still counts");
    assert!(hydrations <= 7, "the oversized row must never decode");
}

#[tokio::test]
async fn an_oversized_neighbor_in_the_page_still_refuses() {
    let (dir, store, scope) = seeded_store();
    for (index, bytes) in [
        (0usize, 0usize),
        (1, super::hydration_support::OVERSIZED_BYTES),
        (2, 0),
    ] {
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
    wide_children(&store, &scope);
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
    assert_eq!(trace_ids(&result), order[..1].to_vec());
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
        [
            "req_bridge_00",
            "req_bridge_01",
            "req_bridge_02",
            "req_dangler",
            "domain_payroll",
            "boundary_no_backpay"
        ]
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
        append_requirement(
            &store,
            &scope,
            &format!("req_island_{index:02}"),
            None,
            &[],
            0,
        );
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
        [
            (1usize, "req_wide_00".to_string()),
            (1, "req_wide_01".to_string()),
            (1, "req_wide_02".to_string()),
            (1, "req_wide_03".to_string())
        ]
    );
    assert!(result.has_more);
    assert!(hydrations <= 7, "a full page must not decode depth two");
}
