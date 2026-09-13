//! The served order of a walk is fixed: node rank, canonical id,
//! declaration order, out before in; and it holds still while records
//! come and go around the survivors.

use super::comparison::requests;
use super::comparison::test_stores::TestStore;
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::{Direction, Neighbor};

fn labels(neighbors: &[Neighbor]) -> Vec<(String, Direction, String)> {
    neighbors
        .iter()
        .map(|neighbor| {
            (
                neighbor.relation.clone(),
                neighbor.direction,
                neighbor.node.id().as_str().to_string(),
            )
        })
        .collect()
}

fn out(relation: &str, id: &str) -> (String, Direction, String) {
    (relation.into(), Direction::Out, id.into())
}

fn into(relation: &str, id: &str) -> (String, Direction, String) {
    (relation.into(), Direction::In, id.into())
}

#[tokio::test]
async fn neighbors_keep_the_rank_id_declaration_direction_order() {
    let store = TestStore::pinned();
    let answer = queries::neighbors(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        requests::neighbors("req_penalty", 50),
    )
    .await
    .unwrap();
    assert_eq!(
        labels(&answer.result.neighbors),
        [
            out("refines", "req_overtime"),
            out("depends_on", "req_overtime"),
            into("requirement_ids", "res_penalty"),
            into("requirement_ids", "rule_penalty_001"),
            into("contradicts", "question_threshold"),
            out("domain_id", "domain_payroll"),
        ],
        "only current records appear"
    );
    assert!(!answer.result.has_more);
    assert_eq!(
        answer.stamp.attested,
        [
            "domains",
            "questions",
            "relations",
            "requirements",
            "resolutions",
            "rules",
            "sources"
        ],
        "the kind probe reads sources first; every table behind an endpoint is named"
    );
    assert!(answer.stamp.live.is_empty());
}
