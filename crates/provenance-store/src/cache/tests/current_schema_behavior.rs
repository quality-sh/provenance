use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::{create_requirement, seeded_layout};
use crate::cache::{catch_up_state, materialize_state, open_cache};
use crate::current_schema::{self, Compatibility};
use crate::state_store::StateStore;
use provenance_macros::verifies;

async fn schema_digest(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_scalar("SELECT schema_digest FROM _cache_metadata WHERE only_row = 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn fresh_creation_installs_one_current_schema_without_a_migration_ledger() {
    let (_dir, layout, _scope) = seeded_layout();

    let report = materialize_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    let ledger: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_schema_migrations'",
    )
    .fetch_optional(cache.pool())
    .await
    .unwrap();

    assert!(report.cache_recreated);
    assert!(ledger.is_none());
    assert!(schema_digest(cache.pool()).await.starts_with("sha256:"));
    cache.close().await.unwrap();
}

#[tokio::test]
async fn a_compatible_cache_keeps_incremental_catch_up() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    create_requirement(
        &StateStore::new(layout.clone()),
        &scope,
        "req_incremental",
        provenance_core::RequirementStatus::Active,
    );

    let report = catch_up_state(&layout).await.unwrap();

    assert!(!report.rebuilt);
    assert!(!report.cache_recreated);
    assert!(report.families_rederived > 0);
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn an_incompatible_cache_is_recreated_from_canonical_state() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    sqlx::query("UPDATE _cache_metadata SET schema_digest = 'incompatible'")
        .execute(cache.pool())
        .await
        .unwrap();
    cache.close().await.unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    let requirements: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requirements")
        .fetch_one(cache.pool())
        .await
        .unwrap();

    assert!(report.rebuilt);
    assert!(report.cache_recreated);
    assert_eq!(requirements, 1);
    assert_ne!(schema_digest(cache.pool()).await, "incompatible");
    cache.close().await.unwrap();
}

#[tokio::test]
async fn a_current_digest_does_not_hide_a_missing_table() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    sqlx::query("DROP TABLE relations")
        .execute(cache.pool())
        .await
        .unwrap();

    assert_eq!(
        current_schema::compatibility(cache.pool()).await.unwrap(),
        Compatibility::RebuildRequired
    );
    cache.close().await.unwrap();
}

#[tokio::test]
async fn a_current_digest_does_not_hide_a_missing_index() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    sqlx::query("DROP INDEX idx_relations_in")
        .execute(cache.pool())
        .await
        .unwrap();

    assert_eq!(
        current_schema::compatibility(cache.pool()).await.unwrap(),
        Compatibility::RebuildRequired
    );
    cache.close().await.unwrap();
}

#[tokio::test]
#[verifies("rule_interrupted_migration_reloads_every_family", examples)]
async fn an_interrupted_rebuild_does_not_publish_the_current_schema_digest() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    sqlx::query("UPDATE _cache_metadata SET schema_digest = 'incompatible'")
        .execute(cache.pool())
        .await
        .unwrap();
    cache.close().await.unwrap();

    crate::test_probes::crash_at("materialize_before_commit");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("materialize_before_commit");
    assert!(error.to_string().contains("injected crash"), "{error:#}");

    let cache = open_cache(&layout).await.unwrap();
    assert_eq!(schema_digest(cache.pool()).await, "incompatible");
    cache.close().await.unwrap();

    let healed = catch_up_state(&layout).await.unwrap();
    assert!(healed.rebuilt);
    assert!(healed.cache_recreated);
}
