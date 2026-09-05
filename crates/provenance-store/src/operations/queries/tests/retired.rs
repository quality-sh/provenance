//! A retired record is answered only when the request asks for it, on
//! every operation that hands records back, and the stamp names the
//! tables that decided it.

use super::comparison::requests;
use super::comparison::test_stores::TestStore;
use crate::operations::queries;
use provenance_core::protocol::{Direction, GetQuery, TracedNode, SDK_PROTOCOL_VERSION};
use provenance_core::NodeType;
use provenance_macros::verifies;

fn get(id: &str, include_retired: bool) -> GetQuery {
    GetQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        node_type: NodeType::Requirement,
        id: id.into(),
        include_retired,
    }
}

#[tokio::test]
#[verifies("rule_retired_records_answer_only_when_asked", examples)]
async fn get_reads_a_retired_record_only_when_asked() {
    let store = TestStore::pinned();
    let hidden = queries::get(
        Some(store.root.clone()),
        &store.scope,
        get("req_old_overtime", false),
    )
    .await
    .unwrap();
    assert!(
        !hidden.result.found,
        "a retired record is out of an active view"
    );
    assert!(hidden.result.node.is_none());
    assert_eq!(hidden.stamp.attested, ["requirements"]);
    assert!(hidden.stamp.live.is_empty(), "get reads nothing live");

    let shown = queries::get(
        Some(store.root.clone()),
        &store.scope,
        get("req_old_overtime", true),
    )
    .await
    .unwrap();
    assert!(shown.result.found);
    assert!(shown.result.node.unwrap().retired());
    assert_eq!(shown.stamp.attested, ["requirements"]);
}

/// A retired origin named with its kind still answers the live records
/// that point at it; its own fields are not followed, so the domain it
/// names is neither answered nor read.
#[tokio::test]
#[verifies("rule_retired_records_answer_only_when_asked", examples)]
async fn a_retired_origin_still_answers_its_live_in_neighbours() {
    let store = TestStore::pinned();
    let mut request = requests::neighbors("req_right", false, 50);
    request.node_type = Some(NodeType::Requirement);
    let answer = queries::neighbors(Some(store.root.clone()), &store.scope, request)
        .await
        .unwrap();
    let labels: Vec<(&str, Direction, &str)> = answer
        .result
        .neighbors
        .iter()
        .map(|n| (n.relation.as_str(), n.direction, n.node.id().as_str()))
        .collect();
    assert_eq!(labels, [("depends_on", Direction::In, "req_bottom")]);
    assert_eq!(answer.stamp.attested, ["relations", "requirements"]);
}

fn traced(nodes: &[TracedNode]) -> Vec<(usize, &str)> {
    nodes
        .iter()
        .map(|node| (node.depth, node.node.id().as_str()))
        .collect()
}

/// `req_bottom` depends on `req_left` and the retired `req_right`, both
/// refining `req_top`. The retired side is marked seen and dropped, so
/// `req_top` is reached once through the live side; asked for, the
/// retired side answers at depth one.
#[tokio::test]
#[verifies("rule_retired_records_answer_only_when_asked", examples)]
async fn a_diamond_over_a_retired_node_answers_as_today() {
    let store = TestStore::pinned();
    let request = |include_retired| {
        let mut request = requests::trace("req_bottom", include_retired, 50);
        request.direction = Direction::Out;
        request
    };
    let active = queries::trace(Some(store.root.clone()), &store.scope, request(false))
        .await
        .unwrap();
    assert_eq!(
        traced(&active.result.nodes),
        [(1, "req_left"), (1, "domain_payroll"), (2, "req_top")]
    );
    assert_eq!(
        active.stamp.attested,
        ["domains", "relations", "requirements", "sources"]
    );
    let all = queries::trace(Some(store.root.clone()), &store.scope, request(true))
        .await
        .unwrap();
    assert_eq!(
        traced(&all.result.nodes),
        [
            (1, "req_left"),
            (1, "req_right"),
            (1, "domain_payroll"),
            (2, "req_top")
        ]
    );
}
