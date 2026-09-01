//! Catch-up keeps rebuild's gate: the aggregate validator runs before any
//! commit, and structural events — a departed scope, a schema move, a lost
//! database — route to the behavior rebuild would show.

use super::super::*;
use super::catch_up_behavior::{assert_catch_up_equals_rebuild, dump_family_tables};
use super::fixtures::*;
use super::projection_digest_sensitivity::aggregate_layout;

async fn stored_serial(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COALESCE(MAX(serial), 0) FROM projection_revision")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn catch_up_refuses_state_the_aggregate_validator_refuses() {
    let (_dir, layout, _scope) = aggregate_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let serial_before = stored_serial(&pool).await;
    let rows_before = dump_family_tables(&pool).await;
    pool.close().await;

    // Remove the allowed disposition actor. No family shard byte moves,
    // but the aggregate rebuild would refuse is now on disk.
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.clear();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let error = catch_up_state(&layout).await.unwrap_err().to_string();
    assert!(error.contains("actor"), "validator error expected: {error}");

    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(stored_serial(&pool).await, serial_before, "no new revision");
    assert_eq!(dump_family_tables(&pool).await, rows_before, "no row moved");
}

#[tokio::test]
async fn a_departed_scope_loses_its_rows_and_its_baselines() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    // The scope leaves the manifest; its shard files remain on disk.
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.clear();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    let pool = open_cache(&layout).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requirements WHERE scope_id = ?")
        .bind(scope.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    let baselines: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM projection_family_digests WHERE scope_id = ?")
            .bind(scope.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 0, "departed scope rows must go");
    assert_eq!(baselines, 0, "departed scope baselines must go");
}

#[tokio::test]
async fn a_schema_move_routes_catch_up_to_a_full_rebuild() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    // Rewind the schema to its pre-019 shape, as every W1 database is.
    let pool = open_cache(&layout).await.unwrap();
    sqlx::query("ALTER TABLE projection_family_digests DROP COLUMN content_digest")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _schema_migrations WHERE id = '019'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt, "a migration must force a rebuild");
    assert!(report.migrations_applied.contains(&"019".to_string()));
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_lost_database_with_a_live_journal_reseeds_above_the_tail() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = crate::state_store::StateStore::new(layout.clone());
    create_source(&store, &scope, "source_after_loss");
    let tail_high = crate::publication::events_in_window(&layout, 1, i64::MAX)
        .unwrap()
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap();

    std::fs::remove_file(layout.cache_db_path()).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt);
    assert!(
        report.serial > tail_high,
        "rebuild serial {} must exceed the surviving tail {tail_high}",
        report.serial
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_concurrent_rebuild_and_catch_up_serialize_on_the_guard() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    let rebuild_layout = layout.clone();
    let catch_up_layout = layout.clone();
    let rebuild = tokio::spawn(async move { materialize_state(&rebuild_layout).await });
    let catch_up = tokio::spawn(async move { catch_up_state(&catch_up_layout).await });
    rebuild.await.unwrap().unwrap();
    catch_up.await.unwrap().unwrap();

    let pool = open_cache(&layout).await.unwrap();
    let serials: Vec<i64> =
        sqlx::query_scalar("SELECT serial FROM projection_revision ORDER BY serial")
            .fetch_all(&pool)
            .await
            .unwrap();
    let mut strictly = serials.clone();
    strictly.dedup();
    assert_eq!(serials, strictly, "one serial progression, no duplicates");
    pool.close().await;
    assert_catch_up_equals_rebuild(&layout).await;
}
