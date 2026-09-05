//! A failed freshness step answers at the stored serial and says so; no
//! revision at all refuses and names `provenance materialize`.

use super::super::differential::corpus;
use super::get_through;
use crate::cache::catch_up_state;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_core::protocol::StampPolicy;

/// Empties the requirement list of every rule in the shard, which the
/// graph validator refuses: a rule needs one requirement.
fn orphan_every_rule(corpus: &corpus::Corpus) {
    let path = crate::shards::rules_path(&corpus.layout(), &corpus.scope);
    let rewritten: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|line| {
            let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
            record["requirement_ids"] = serde_json::json!([]);
            record.to_string()
        })
        .collect();
    std::fs::write(&path, format!("{}\n", rewritten.join("\n"))).unwrap();
}

#[tokio::test]
async fn a_read_answers_at_the_stored_serial_when_catch_up_refuses() {
    let corpus = corpus::seeded_queries();
    crate::cache::tests::fixtures::create_requirement(
        &corpus.store(),
        &corpus.scope,
        "req_rule_anchor",
        provenance_core::RequirementStatus::Active,
    );
    crate::cache::tests::fixtures::create_rule_of(
        &corpus.store(),
        &corpus.scope,
        "rule_anchored",
        "req_rule_anchor",
    );
    let healthy = get_through(&corpus, ReadPolicy::default()).await.unwrap();
    assert_eq!(healthy.stamp.policy, StampPolicy::CatchUp);

    orphan_every_rule(&corpus);
    let refused = catch_up_state(&corpus.layout()).await.unwrap_err();
    assert!(refused.to_string().contains("needs one requirement"));

    let stamped = get_through(&corpus, ReadPolicy::default()).await.unwrap();
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
async fn a_read_with_no_database_refuses_and_names_materialize() {
    let corpus = corpus::seeded_queries();
    let refused = get_through(
        &corpus,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap_err();
    assert!(
        refused.to_string().contains("provenance materialize"),
        "{refused}"
    );
    assert!(!corpus.layout().cache_db_path().exists());
}

#[tokio::test]
async fn a_refused_catch_up_over_no_database_names_materialize() {
    let corpus = corpus::seeded_queries();
    crate::cache::tests::fixtures::create_rule_of(
        &corpus.store(),
        &corpus.scope,
        "rule_anchored",
        "req_overtime",
    );
    orphan_every_rule(&corpus);
    let refused = get_through(&corpus, ReadPolicy::default())
        .await
        .unwrap_err();
    let text = format!("{refused:#}");
    assert!(text.contains("provenance materialize"), "{text}");
    assert!(text.contains("needs one requirement"), "{text}");
}

#[tokio::test]
async fn refuse_stale_is_reserved() {
    let corpus = corpus::seeded_queries();
    let refused = get_through(
        &corpus,
        ReadPolicy::with_freshness(FreshnessPolicy::RefuseStale),
    )
    .await
    .unwrap_err();
    assert!(refused.to_string().contains("refuse_stale"), "{refused}");
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

    let corpus = corpus::seeded_queries();
    let healthy = get_through(&corpus, ReadPolicy::default()).await.unwrap();

    let cache = corpus.layout().cache_dir();
    let _restore = Writable(cache.clone());
    std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(cache.join("probe"), b"").is_ok() {
        // Running as a user that ignores directory permissions.
        return;
    }

    let stamped = get_through(&corpus, ReadPolicy::default()).await.unwrap();
    assert!(stamped.result.found);
    assert_eq!(stamped.stamp.policy, StampPolicy::CatchUpFailed);
    assert_eq!(stamped.stamp.serial, healthy.stamp.serial);
    assert!(stamped.freshness_error.is_some());
}

/// A cache file from before migration 018: the migration table stops at
/// 017 and no revision table exists.
async fn old_database(corpus: &corpus::Corpus) {
    use sqlx::{Connection, SqliteConnection};
    let layout = corpus.layout();
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
    let corpus = corpus::seeded_queries();
    old_database(&corpus).await;
    let refused = get_through(
        &corpus,
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
    let corpus = corpus::seeded_queries();
    old_database(&corpus).await;
    // A file where the lock directory belongs makes the guard fail before
    // catch-up can run a migration.
    let locks = corpus.layout().cache_dir().join("locks");
    std::fs::remove_dir_all(&locks).unwrap();
    std::fs::write(&locks, b"").unwrap();
    let refused = get_through(&corpus, ReadPolicy::default())
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
