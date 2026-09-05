//! The typed table handles read the projection back as the records the
//! store wrote: byte for byte, past the bind limit, by kind rank, and
//! with retired rows only when asked.

use super::comparison::test_stores::TestStore;
use crate::cache::read::kind_of;
use crate::cache::tests::fixtures::pinned_store::TWIN_ID;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::reader::ReadSnapshot;
use provenance_core::model::ProjectionRow;
use provenance_core::{
    Boundary, Domain, ImplementationBinding, NodeType, Question, Requirement, RequirementReview,
    Resolution, Rule, Source, StableId, Topic, VerificationBinding,
};
use serde::Serialize;
use sqlx::SqlitePool;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

async fn snapshot_of(store: &TestStore) -> (SqlitePool, ReadSnapshot) {
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    let snapshot = ReadSnapshot::open(&pool, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    (pool, snapshot)
}

/// Every canonical record of one kind reads back from its table as the
/// same JSON the store serializes.
async fn assert_reads_back<K: ProjectionRow + Serialize>(snapshot: &ReadSnapshot, records: Vec<K>) {
    assert!(
        !records.is_empty(),
        "{}: the store must seed the kind",
        K::TABLE
    );
    let table = snapshot.table::<K>();
    for record in records {
        let value = serde_json::to_value(&record).unwrap();
        let id = sid(value["id"].as_str().unwrap());
        let read = table
            .record(&id)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{}: {} is not in the table", K::TABLE, id.as_str()));
        assert_eq!(
            serde_json::to_string(&read).unwrap(),
            serde_json::to_string(&record).unwrap(),
            "{}: {} reads back differently",
            K::TABLE,
            id.as_str()
        );
    }
}

#[tokio::test]
async fn a_stored_record_reads_back_as_its_canonical_bytes() {
    let store = TestStore::pinned();
    let state = store.state_store();
    let scope = &store.scope;
    let (pool, snapshot) = snapshot_of(&store).await;
    assert_reads_back::<Source>(&snapshot, state.list_sources(scope).unwrap()).await;
    assert_reads_back::<Requirement>(&snapshot, state.list_requirements(scope).unwrap()).await;
    assert_reads_back::<Resolution>(&snapshot, state.list_resolutions(scope).unwrap()).await;
    assert_reads_back::<Rule>(&snapshot, state.list_rules(scope).unwrap()).await;
    assert_reads_back::<Topic>(&snapshot, state.list_topics(scope).unwrap()).await;
    assert_reads_back::<Question>(&snapshot, state.list_questions(scope).unwrap()).await;
    assert_reads_back::<Domain>(&snapshot, state.list_domains(scope).unwrap()).await;
    assert_reads_back::<Boundary>(&snapshot, state.list_boundaries(scope).unwrap()).await;
    assert_reads_back::<ImplementationBinding>(
        &snapshot,
        state.list_implementation_bindings(scope).unwrap(),
    )
    .await;
    assert_reads_back::<VerificationBinding>(
        &snapshot,
        state.list_verification_bindings(scope).unwrap(),
    )
    .await;
    assert_reads_back::<RequirementReview>(
        &snapshot,
        state.list_requirement_reviews(scope).unwrap(),
    )
    .await;
    drop(snapshot);
    pool.close().await;
}

fn ids<K: Serialize>(records: &[K]) -> Vec<String> {
    records
        .iter()
        .map(|record| {
            serde_json::to_value(record).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn search_reads_a_retired_record_only_when_asked_and_orders_by_id() {
    let store = TestStore::pinned();
    let (pool, snapshot) = snapshot_of(&store).await;
    let requirements = snapshot.table::<Requirement>();
    assert_eq!(
        ids(&requirements.search("Penalty", false).await.unwrap()),
        ["req_penalty"],
        "the needle is folded to lowercase"
    );
    assert!(requirements.search("old", false).await.unwrap().is_empty());
    assert_eq!(
        ids(&requirements.search("old", true).await.unwrap()),
        ["req_old_overtime"]
    );
    assert_eq!(
        ids(&requirements.search("overtime", true).await.unwrap()),
        [
            "req_bottom",
            "req_left",
            "req_old_overtime",
            "req_overtime",
            "req_penalty",
            "req_right",
            "req_top",
            "twin_record",
        ]
    );
    assert!(requirements
        .search("no such text", true)
        .await
        .unwrap()
        .is_empty());
    drop(snapshot);
    pool.close().await;
}

/// `SQLite` bounds the bind parameters of one statement; a lookup over
/// more ids than that bound still answers.
#[tokio::test]
async fn by_ids_reads_past_the_bind_limit() {
    let store = TestStore::pinned();
    let (pool, snapshot) = snapshot_of(&store).await;
    let mut wanted: Vec<StableId> = (0..40_000)
        .map(|n| sid(&format!("rule_none_{n}")))
        .collect();
    wanted.push(sid("rule_overtime_001"));
    wanted.insert(7, sid("rule_penalty_001"));
    let found = ids(&snapshot.table::<Rule>().by_ids(&wanted).await.unwrap());
    assert_eq!(found, ["rule_overtime_001", "rule_penalty_001"]);
    drop(snapshot);
    pool.close().await;
}

#[tokio::test]
async fn kind_of_reads_kinds_in_rank_order_and_skips_retired() {
    let store = TestStore::pinned();
    let (pool, snapshot) = snapshot_of(&store).await;
    assert_eq!(
        kind_of(&snapshot, &sid(TWIN_ID), false).await.unwrap(),
        Some(NodeType::Requirement),
        "a requirement outranks a rule of the same id"
    );
    assert_eq!(
        kind_of(&snapshot, &sid("req_old_overtime"), false)
            .await
            .unwrap(),
        None,
        "a retired record has no kind in an active view"
    );
    assert_eq!(
        kind_of(&snapshot, &sid("req_old_overtime"), true)
            .await
            .unwrap(),
        Some(NodeType::Requirement)
    );
    assert_eq!(
        kind_of(&snapshot, &sid("boundary_no_backpay"), false)
            .await
            .unwrap(),
        Some(NodeType::Boundary)
    );
    assert_eq!(
        kind_of(&snapshot, &sid("nobody"), true).await.unwrap(),
        None
    );
    drop(snapshot);
    pool.close().await;
}
