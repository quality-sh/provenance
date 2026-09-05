//! The served order of a walk is fixed: node rank, canonical id,
//! declaration order, out before in; and it holds still while records
//! come and go around the survivors.

use super::comparison::requests;
use super::comparison::test_stores::TestStore;
use crate::cache::tests::fixtures::{create_rule_of, pinned_store::mark_retired};
use crate::operations::queries;
use crate::shards;
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
        requests::neighbors("req_penalty", false, 50),
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
        "the retired req_old_overtime it supersedes is left out"
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

/// A rule appears at its slot and a retired one leaves; the survivors
/// keep their relative order.
#[tokio::test]
async fn survivors_keep_their_order_when_records_are_inserted_and_retired() {
    let store = TestStore::pinned();
    let request = || requests::neighbors("req_overtime", false, 50);
    let before = labels(
        &queries::neighbors(Some(store.root.clone()), &store.scope, request())
            .await
            .unwrap()
            .result
            .neighbors,
    );
    assert!(before.contains(&into("requirement_ids", "rule_over_005")));

    create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_aaa_new",
        "req_overtime",
    );
    mark_retired(
        &shards::rules_path(&store.layout(), &store.scope),
        "rule_over_005",
    );
    let after = labels(
        &queries::neighbors(Some(store.root.clone()), &store.scope, request())
            .await
            .unwrap()
            .result
            .neighbors,
    );
    let survivors: Vec<_> = after
        .iter()
        .filter(|label| label.2 != "rule_aaa_new")
        .cloned()
        .collect();
    let expected: Vec<_> = before
        .iter()
        .filter(|label| label.2 != "rule_over_005")
        .cloned()
        .collect();
    assert_eq!(survivors, expected, "the survivors keep their order");
    let slot = after
        .iter()
        .position(|label| label.2 == "rule_aaa_new")
        .expect("the new rule is answered");
    assert_eq!(
        after[slot - 1].2,
        "res_overtime",
        "a rule sits after the resolutions"
    );
    assert_eq!(
        after[slot + 1].2,
        "rule_over_002",
        "and before the rules that sort after it"
    );
}
