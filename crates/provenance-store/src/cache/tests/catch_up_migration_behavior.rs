//! A migration that recreates tables commits before the rebuild that
//! refills them. A crash in between must not leave the tables empty at a
//! serial that claims them full: the next pass reloads every family.

use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::catch_up_serial_behavior::latest_revision;
use super::fixtures::seeded_layout;
use crate::cache::{catch_up_state, materialize_state, open_cache};
use crate::migrations::RECORD_COLUMNS_MIGRATION_ID;

async fn requirement_count(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM requirements")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Forgetting that 022 ran makes the next pass apply it again, which drops
/// and recreates the eleven tables: the shape of a migration over a live
/// database.
async fn forget_migration_022(pool: &sqlx::SqlitePool) {
    sqlx::query("DELETE FROM _schema_migrations WHERE id = ?")
        .bind(RECORD_COLUMNS_MIGRATION_ID)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_crash_between_a_migration_and_its_rebuild_is_healed_by_the_next_pass() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (serial_before, _) = latest_revision(&pool).await;
    forget_migration_022(&pool).await;
    pool.close().await;

    crate::test_probes::crash_at("catch_up_after_migrations");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("catch_up_after_migrations");
    assert!(error.to_string().contains("injected crash"), "{error}");

    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(
        requirement_count(&pool).await,
        0,
        "the migration committed and emptied the table before the crash"
    );
    pool.close().await;

    let report = catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(requirement_count(&pool).await, 1, "{report:?}");
    let (serial_after, _) = latest_revision(&pool).await;
    assert!(serial_after > serial_before, "{report:?}");
    pool.close().await;
    assert_catch_up_equals_rebuild(&layout).await;
}
