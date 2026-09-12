//! The typed table handles read the projection back as the records the
//! store wrote: byte for byte, past the bind limit, by kind rank, and
//! from the current canonical state.

use super::comparison::test_stores::{self, TestStore};
use crate::cache::read::{column_values, select_columns};
use crate::cache::{catch_up_state, open_cache, quoted, CacheConnection};
use crate::operations::reader::ReadSnapshot;
use provenance_core::model::ProjectionRow;
use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, Requirement, RequirementReview, Resolution,
    Rule, Source, StableId, Topic, VerificationBinding,
};
use provenance_macros::verifies;
use serde::Serialize;
use sqlx::SqlitePool;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

async fn snapshot_of(store: &TestStore) -> (CacheConnection, ReadSnapshot) {
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    let snapshot = ReadSnapshot::open(pool.pool(), &store.scope)
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
#[verifies("rule_record_reads_back_from_its_row_identical", examples)]
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
    pool.close().await.unwrap();
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
    assert_rows_read_as_written::<Source>(pool.pool(), word, state.list_sources(scope).unwrap())
        .await;
    assert_rows_read_as_written::<Requirement>(
        pool.pool(),
        word,
        state.list_requirements(scope).unwrap(),
    )
    .await;
    assert_rows_read_as_written::<Resolution>(pool.pool(), word, resolutions).await;
    assert_rows_read_as_written::<Rule>(pool.pool(), word, state.list_rules(scope).unwrap()).await;
    assert_rows_read_as_written::<Topic>(pool.pool(), word, state.list_topics(scope).unwrap())
        .await;
    assert_rows_read_as_written::<Question>(
        pool.pool(),
        word,
        state.list_questions(scope).unwrap(),
    )
    .await;
    assert_rows_read_as_written::<Domain>(pool.pool(), word, state.list_domains(scope).unwrap())
        .await;
    assert_rows_read_as_written::<Boundary>(
        pool.pool(),
        word,
        state.list_boundaries(scope).unwrap(),
    )
    .await;
    pool.close().await.unwrap();
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
    pool.close().await.unwrap();
}

/// One record per id: an id that repeats across two chunks is still one
/// row in the answer.
#[tokio::test]
#[verifies("rule_by_ids_answers_a_repeated_id_once", examples)]
async fn by_ids_reads_a_repeated_id_once_across_chunks() {
    let store = TestStore::pinned();
    let (pool, snapshot) = snapshot_of(&store).await;
    let mut wanted: Vec<StableId> = (0..499).map(|n| sid(&format!("rule_none_{n}"))).collect();
    wanted.push(sid("rule_overtime_001"));
    wanted.push(sid("rule_overtime_001"));
    wanted.push(sid("rule_penalty_001"));
    let found = ids(&snapshot.table::<Rule>().by_ids(&wanted).await.unwrap());
    assert_eq!(found, ["rule_overtime_001", "rule_penalty_001"]);
    drop(snapshot);
    pool.close().await.unwrap();
}
