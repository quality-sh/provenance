use super::*;
use crate::operations::read_policy::FreshnessPolicy;
use provenance_core::protocol::{ReadDocumentResult, Stamped};

async fn read(root: &camino::Utf8Path, freshness: FreshnessPolicy) -> Stamped<ReadDocumentResult> {
    queries::read_document(
        &repository_at(root),
        ReadPolicy::with_freshness(freshness),
        ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal: false,
            cursor: None,
            limit: 50,
        },
    )
    .await
    .unwrap()
}

fn outcome(answer: &Stamped<ReadDocumentResult>, id: &str) -> Value {
    review(&serde_json::to_value(&answer.result).unwrap(), id)["outcome"].clone()
}

/// Implementation aid: a page keeps the review state of a revision only
/// after a freshness step matched the revision to canonical state. Under
/// `annotate_only` the page reads canonical state again, so it shows a
/// decision that no catch-up saw.
#[tokio::test]
async fn annotate_only_page_shows_a_decision_that_no_catch_up_saw() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    create_document_records(&store, &scope);
    let root = root_of(&dir);
    let id = sid("rule_policy");
    let proposal = submit(&store, &scope, NodeType::Rule, &id);
    let before = read(&root, FreshnessPolicy::CatchUp).await;
    assert_eq!(outcome(&before, "rule_policy"), "pending");

    decide(
        &store,
        &scope,
        NodeType::Rule,
        &id,
        proposal,
        DispositionDecision::Accepted,
    );

    let stale = read(&root, FreshnessPolicy::AnnotateOnly).await;
    assert_eq!(stale.stamp.serial, before.stamp.serial);
    assert_eq!(outcome(&stale, "rule_policy"), "accepted");
    let current = read(&root, FreshnessPolicy::CatchUp).await;
    assert!(current.stamp.serial > before.stamp.serial);
    assert_eq!(outcome(&current, "rule_policy"), "accepted");
}
