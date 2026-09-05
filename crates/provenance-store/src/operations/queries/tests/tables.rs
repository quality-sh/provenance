//! The typed table handles read the projection back as the records the
//! store wrote: byte for byte, past the bind limit, by kind rank, and
//! with retired rows only when asked.

use super::comparison::test_stores::{self, TestStore};
use crate::cache::read::{column_values, kind_of, select_columns};
use crate::cache::tests::fixtures::pinned_store::TWIN_ID;
use crate::cache::{catch_up_state, open_cache, quoted};
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

/// Every stored row of one kind reads back as the values the derive
/// encoded, storage class included. The record's bytes cannot show a
/// class swap: serde reads the JSON `1` an integer column gives into an
/// `f64` field as `1.0` all the same, so this compares the values
/// themselves.
async fn assert_rows_read_as_written<K: ProjectionRow + Serialize>(
    pool: &SqlitePool,
    scope: &str,
    records: Vec<K>,
) {
    assert!(
        !records.is_empty(),
        "{}: the store must seed the kind",
        K::TABLE
    );
    let sql = format!(
        "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
        select_columns::<K>(),
        quoted(K::TABLE)
    );
    for record in records {
        let id = serde_json::to_value(&record).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let row = sqlx::query(&sql)
            .bind(scope)
            .bind(&id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            column_values::<K>(&row).unwrap(),
            record.row().unwrap(),
            "{}: {id} reads back in other storage classes",
            K::TABLE
        );
    }
}

/// The repository's own state holds a real in a `REAL` column (a
/// resolution confidence of one) that no seeded fixture writes, so this is
/// the store-side twin of `a_round_confidence_stays_a_float`: an integral
/// real read back as an integer fails here.
#[tokio::test]
async fn every_stored_row_of_the_repository_state_reads_back_as_written() {
    let store = test_stores::repository_state();
    let state = store.state_store();
    let scope = &store.scope;
    let resolutions = state.list_resolutions(scope).unwrap();
    assert!(
        resolutions
            .iter()
            .any(|resolution| resolution.confidence == Some(1.0)),
        "the repository state must hold a resolution with confidence 1.0"
    );
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    let word = scope.as_str();
    assert_rows_read_as_written::<Source>(&pool, word, state.list_sources(scope).unwrap()).await;
    assert_rows_read_as_written::<Requirement>(
        &pool,
        word,
        state.list_requirements(scope).unwrap(),
    )
    .await;
    assert_rows_read_as_written::<Resolution>(&pool, word, resolutions).await;
    assert_rows_read_as_written::<Rule>(&pool, word, state.list_rules(scope).unwrap()).await;
    assert_rows_read_as_written::<Topic>(&pool, word, state.list_topics(scope).unwrap()).await;
    assert_rows_read_as_written::<Question>(&pool, word, state.list_questions(scope).unwrap())
        .await;
    assert_rows_read_as_written::<Domain>(&pool, word, state.list_domains(scope).unwrap()).await;
    assert_rows_read_as_written::<Boundary>(&pool, word, state.list_boundaries(scope).unwrap())
        .await;
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
