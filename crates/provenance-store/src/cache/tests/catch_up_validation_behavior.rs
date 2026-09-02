//! Catch-up keeps rebuild's gate. The aggregate validator runs before any
//! commit. A departed scope, a schema move, and a lost database behave as
//! they do in a rebuild.

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

    // Remove the allowed disposition actor. No shard byte moves, but the
    // aggregate on disk is one a rebuild refuses.
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

    // The scope leaves the manifest. Its shard files remain on disk.
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

    // Rewind the schema to its pre-020 shape.
    let pool = open_cache(&layout).await.unwrap();
    for statement in [
        "DROP TABLE projection_unit_digests",
        "ALTER TABLE projection_family_digests ADD COLUMN digest TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE projection_family_digests ADD COLUMN size_bytes INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE projection_family_digests ADD COLUMN mtime_ns INTEGER NOT NULL DEFAULT 0",
    ] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    sqlx::query("DELETE FROM _schema_migrations WHERE id = '020'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt, "a migration must force a rebuild");
    assert!(report.migrations_applied.contains(&"020".to_string()));
    assert_catch_up_equals_rebuild(&layout).await;
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
