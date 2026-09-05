//! Every operation stops at the limit and says `has_more`; nothing pages.

use super::comparison::requests;
use super::comparison::test_stores::TestStore;
use crate::operations::queries;
use provenance_core::NodeType;
use provenance_macros::verifies;

/// The pinned store holds six active requirements whose statements say
/// "overtime" and no source that does, so a limit of two is met inside
/// the requirements table and the rules table is never read.
#[tokio::test]
#[verifies("rule_query_answers_stop_at_the_limit", examples)]
async fn search_visits_kinds_in_rank_order_and_stops_at_the_limit() {
    let store = TestStore::pinned();
    let mut request = requests::search("overtime", Vec::new());
    request.limit = 2;
    let answer = queries::search(Some(store.root.clone()), &store.scope, request)
        .await
        .unwrap();
    let ids: Vec<&str> = answer
        .result
        .nodes
        .iter()
        .map(|node| node.id().as_str())
        .collect();
    assert_eq!(ids, ["req_bottom", "req_left"]);
    assert!(answer.result.has_more);
    assert!(answer
        .result
        .nodes
        .iter()
        .all(|node| node.node_type() == NodeType::Requirement));
    assert_eq!(
        answer.stamp.attested,
        ["requirements", "sources"],
        "the kinds before the limit are read; the kinds after it are not"
    );
    assert!(answer.stamp.live.is_empty());
}

/// `req_top` has three live records one hop away, so a limit of two cuts
/// the first depth; the stamp shows the walk read nothing past it.
#[tokio::test]
#[verifies("rule_query_answers_stop_at_the_limit", examples)]
async fn trace_stops_at_the_limit_and_says_has_more() {
    let store = TestStore::pinned();
    let answer = queries::trace(
        Some(store.root.clone()),
        &store.scope,
        requests::trace("req_top", false, 2),
    )
    .await
    .unwrap();
    let ids: Vec<(usize, &str)> = answer
        .result
        .nodes
        .iter()
        .map(|node| (node.depth, node.node.id().as_str()))
        .collect();
    assert_eq!(ids, [(1, "req_left"), (1, "twin_record")]);
    assert!(answer.result.has_more);
    assert_eq!(
        answer.stamp.attested,
        ["domains", "relations", "requirements", "rules", "sources"]
    );
    assert!(answer.stamp.live.is_empty());
}
