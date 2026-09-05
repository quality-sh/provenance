//! A failed freshness step answers at the stored serial and says so; no
//! revision at all refuses and names `provenance materialize`.

use super::super::comparison::test_stores;
use super::get_through;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_core::protocol::StampPolicy;
use provenance_macros::verifies;

/// Empties the requirement list of every rule in the shard, which the
/// graph validator refuses: a rule needs one requirement.
fn orphan_every_rule(store: &test_stores::TestStore) {
    crate::cache::tests::fixtures::rewrite_records(
        &crate::shards::rules_path(&store.layout(), &store.scope),
        |record| record["requirement_ids"] = serde_json::json!([]),
    );
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_answers_at_the_stored_serial_when_catch_up_refuses() {
    let store = test_stores::seeded_queries();
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        "req_rule_anchor",
        provenance_core::RequirementStatus::Active,
    );
    crate::cache::tests::fixtures::create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_anchored",
        "req_rule_anchor",
    );
    let healthy = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert_eq!(healthy.stamp.policy, StampPolicy::CatchUp);

    orphan_every_rule(&store);
    let refused = catch_up_state(&store.layout()).await.unwrap_err();
    assert!(refused.to_string().contains("needs one requirement"));

    let stamped = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert!(stamped.result.found, "the read still answers");
    assert_eq!(stamped.stamp.policy, StampPolicy::CatchUpFailed);
    assert_eq!(stamped.stamp.serial, healthy.stamp.serial);
    assert_eq!(stamped.stamp.digest, healthy.stamp.digest);
    let error = stamped
        .freshness_error
        .expect("the failed step's error travels with the answer");
    assert!(error.contains("needs one requirement"), "{error}");
}

#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_read_with_no_database_refuses_and_names_materialize() {
    let store = test_stores::seeded_queries();
    let refused = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap_err();
    assert!(
        refused.to_string().contains("provenance materialize"),
        "{refused}"
    );
    assert!(!store.layout().cache_db_path().exists());
}

#[tokio::test]
async fn a_refused_catch_up_over_no_database_names_materialize() {
    let store = test_stores::seeded_queries();
    crate::cache::tests::fixtures::create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_anchored",
        "req_overtime",
    );
    orphan_every_rule(&store);
    let refused = get_through(&store, ReadPolicy::default())
        .await
        .unwrap_err();
    let text = format!("{refused:#}");
    assert!(text.contains("provenance materialize"), "{text}");
    assert!(text.contains("needs one requirement"), "{text}");
}

/// Makes the directory writable again when the test ends, however it ends.
#[cfg(unix)]
struct Writable(camino::Utf8PathBuf);

#[cfg(unix)]
impl Drop for Writable {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// A read from a checkout whose cache directory cannot be written answers
/// at the serial it holds. WAL needs the `-shm` file beside the database,
/// so the reader opens the file as an immutable image.
#[cfg(unix)]
#[tokio::test]
async fn a_read_only_checkout_answers_at_its_serial() {
    use std::os::unix::fs::PermissionsExt;

    let store = test_stores::seeded_queries();
    let healthy = get_through(&store, ReadPolicy::default()).await.unwrap();

    let cache = store.layout().cache_dir();
    let _restore = Writable(cache.clone());
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(cache.join("probe"), b"").is_ok() {
        // Running as a user that ignores directory permissions.
        return;
    }

    let stamped = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert!(stamped.result.found);
    assert_eq!(stamped.stamp.policy, StampPolicy::CatchUpFailed);
    assert_eq!(stamped.stamp.serial, healthy.stamp.serial);
    assert!(stamped.freshness_error.is_some());
}

/// A cache file from before migration 018: the migration table stops at
/// 017 and no revision table exists.
async fn old_database(store: &test_stores::TestStore) {
    use sqlx::{Connection, SqliteConnection};
    let layout = store.layout();
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(layout.cache_db_path())
        .create_if_missing(true);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::query(
        "CREATE TABLE _schema_migrations (id TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    for id in 1..=17 {
        sqlx::query("INSERT INTO _schema_migrations (id) VALUES (?)")
            .bind(format!("{id:03}"))
            .execute(&mut connection)
            .await
            .unwrap();
    }
    connection.close().await.unwrap();
}

#[tokio::test]
async fn annotate_only_refuses_a_database_behind_on_migrations_by_type() {
    let store = test_stores::seeded_queries();
    old_database(&store).await;
    let refused = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            refused.downcast_ref::<crate::operations::reader::ReadRefusal>(),
            Some(crate::operations::reader::ReadRefusal::SchemaBehind { .. })
        ),
        "{refused:#}"
    );
    assert!(refused.to_string().contains("provenance materialize"));
}

/// When the freshness step fails before it can migrate, the read falls
/// back to the stored file; an old file holds no revision table, and the
/// refusal must still be the typed one that names materialize.
#[tokio::test]
async fn a_pre_stamp_database_refuses_and_names_materialize_when_catch_up_fails() {
    let store = test_stores::seeded_queries();
    old_database(&store).await;
    // A file where the lock directory belongs makes the guard fail before
    // catch-up can run a migration.
    let locks = store.layout().cache_dir().join("locks");
    std::fs::remove_dir_all(&locks).unwrap();
    std::fs::write(&locks, b"").unwrap();
    let refused = get_through(&store, ReadPolicy::default())
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused.downcast_ref::<crate::operations::reader::ReadRefusal>(),
            Some(crate::operations::reader::ReadRefusal::NoProjection { .. })
        ),
        "{refused:#}"
    );
    let text = format!("{refused:#}");
    assert!(text.contains("provenance materialize"), "{text}");
    assert!(!text.contains("no such table"), "{text}");
}

/// A migration that committed before its rebuild ran leaves a revision
/// row beside empty tables and no family digests. `catch_up` heals that
/// window before it answers; `annotate_only` runs no freshness step, so
/// it must refuse instead of answering over empty tables.
#[tokio::test]
#[verifies("rule_annotate_only_refuses_a_half_migrated_projection", examples)]
async fn annotate_only_refuses_a_half_migrated_database() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    sqlx::query("DELETE FROM _schema_migrations WHERE id = ?")
        .bind(crate::migrations::RECORD_COLUMNS_MIGRATION_ID)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    crate::test_probes::crash_at("catch_up_after_migrations");
    let crashed = catch_up_state(&store.layout()).await.unwrap_err();
    crate::test_probes::disarm("catch_up_after_migrations");
    assert!(crashed.to_string().contains("injected crash"), "{crashed}");

    let refused = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            refused.downcast_ref::<crate::operations::reader::ReadRefusal>(),
            Some(crate::operations::reader::ReadRefusal::HalfMigrated { .. })
        ),
        "{refused:#}"
    );
    assert!(refused.to_string().contains("provenance materialize"));

    let healed = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert!(healed.result.found, "catch-up heals the window and answers");
}

/// Family digest rows are one per scope, so a manifest that names no
/// scope materializes a revision beside no digest rows. That projection
/// is complete, and `annotate_only` answers over it.
#[tokio::test]
#[verifies("rule_annotate_only_refuses_a_half_migrated_projection", examples)]
async fn a_projection_with_no_scope_is_not_half_migrated() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let manifest_path = store.layout().manifest_path();
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["scopes"] = serde_json::json!([]);
    std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
    catch_up_state(&store.layout()).await.unwrap();
    let answer = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .expect("a complete projection with no scope answers");
    assert!(!answer.result.found, "the scope is gone with its rows");
    assert_eq!(answer.stamp.policy, StampPolicy::AnnotateOnly);
}
