use super::super::*;
use super::catch_up_behavior::dump_family_tables;
use super::fixtures::*;
use super::projection_stamp_behavior::seed_integration_shards;
use crate::publication::events_in_window;
use crate::state_store::StateStore;

async fn stamped(pool: &sqlx::SqlitePool) -> (i64, String, String) {
    let (serial, digest): (i64, String) = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let instance: String = sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_one(pool)
        .await
        .unwrap();
    (serial, digest, instance)
}

fn stamped_layout() -> (
    tempfile::TempDir,
    crate::layout::ProvenanceLayout,
    provenance_core::ScopeId,
) {
    let (dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    (dir, layout, scope)
}

#[tokio::test]
async fn journal_and_head_loss_with_the_database_alive_stays_monotonic() {
    let (_dir, layout, _scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (first_serial, first_digest, _) = stamped(&pool).await;
    drop(pool);

    std::fs::remove_dir_all(layout.cache_dir().join("journal")).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert!(
        report.serial > first_serial,
        "the stored serial floors the head"
    );
    assert_eq!(
        report.digest, first_digest,
        "unchanged state keeps its digest"
    );
    assert_eq!(report.families_hashed, 19);
}

#[tokio::test]
async fn a_truncated_tail_narrows_the_hint_and_changes_nothing_else() {
    let (_dir, layout, scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_kept");
    create_source(&store, &scope, "source_truncated");

    let events_path = layout.cache_dir().join("journal/events.jsonl");
    let content = std::fs::read_to_string(&events_path).unwrap();
    let mut truncated: String = content.lines().next().unwrap().to_string();
    truncated.push_str("\n{\"sequence\":");
    std::fs::write(&events_path, truncated).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.events_drained <= 1, "the torn line is not an event");
    let pool = open_cache(&layout).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3, "the hash sweep found both writes anyway");
}

#[tokio::test]
async fn a_gapped_tail_still_hashes_every_family() {
    let (_dir, layout, scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_one");
    create_requirement(
        &store,
        &scope,
        "req_two",
        provenance_core::RequirementStatus::Active,
    );

    let events_path = layout.cache_dir().join("journal/events.jsonl");
    let content = std::fs::read_to_string(&events_path).unwrap();
    let kept: Vec<&str> = content.lines().skip(1).collect();
    std::fs::write(&events_path, format!("{}\n", kept.join("\n"))).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.families_hashed, 19);
    let pool = open_cache(&layout).await.unwrap();
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    let requirements: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requirements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((sources, requirements), (2, 2));
}

#[tokio::test]
async fn total_cache_loss_restarts_the_serial_inside_a_fresh_instance() {
    let (_dir, layout, _scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (_, first_digest, first_instance) = stamped(&pool).await;
    pool.close().await;
    // Total loss means every durable component: the database and its
    // sidecars, the event log, and the head record beside it.
    for name in [
        "provenance.db",
        "provenance.db-shm",
        "provenance.db-wal",
        "journal.head.json",
    ] {
        let path = layout.cache_dir().join(name);
        if path.exists() {
            std::fs::remove_file(path).unwrap();
        }
    }
    std::fs::remove_dir_all(layout.cache_dir().join("journal")).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt, "a lost database routes to total rebuild");
    assert_eq!(report.serial, 1, "only total cache loss restarts at one");
    assert_eq!(
        report.digest, first_digest,
        "same canonical bytes, same digest"
    );
    let pool = open_cache(&layout).await.unwrap();
    let (_, _, second_instance) = stamped(&pool).await;
    assert_ne!(
        first_instance, second_instance,
        "the restart is scoped to a fresh projection instance"
    );
}

#[tokio::test]
async fn a_crash_before_the_commit_leaves_the_previous_stamp_readable() {
    let (_dir, layout, scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (first_serial, first_digest, _) = stamped(&pool).await;
    let rows_before = dump_family_tables(&pool).await;
    drop(pool);

    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_uncommitted");
    crate::test_probes::crash_at("catch_up_before_commit");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("catch_up_before_commit");
    assert!(error.to_string().contains("injected crash"), "{error}");

    let pool = open_cache(&layout).await.unwrap();
    let (serial, digest, _) = stamped(&pool).await;
    assert_eq!((serial, digest), (first_serial, first_digest));
    assert_eq!(rows_before, dump_family_tables(&pool).await);
    drop(pool);

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.serial > first_serial);
    assert!(
        report.families_rederived >= 1,
        "the write lands on the next pass"
    );
}

#[tokio::test]
async fn a_crash_between_commit_and_head_fsync_recovers_from_the_stored_revision() {
    let (_dir, layout, scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_committed");

    crate::test_probes::crash_at("db_committed_before_head_fsync");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("db_committed_before_head_fsync");
    assert!(error.to_string().contains("injected crash"), "{error}");

    // The transaction committed before the crash; the head and the tail
    // were left behind and must repair from the stored revision.
    let pool = open_cache(&layout).await.unwrap();
    let (committed_serial, _, _) = stamped(&pool).await;
    drop(pool);

    let report = catch_up_state(&layout).await.unwrap();
    assert!(
        report.serial > committed_serial,
        "no sequence at or below the committed serial is reused"
    );
    assert_eq!(
        report.rows_written, 0,
        "re-drained events re-derive idempotently"
    );
    assert!(events_in_window(&layout, 1, report.serial)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_catch_up_after_a_rebuild_sees_its_serial_and_drains_nothing() {
    let (_dir, layout, _scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (rebuild_serial, _, _) = stamped(&pool).await;
    drop(pool);

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert_eq!(report.events_drained, 0);
    assert!(report.serial > rebuild_serial, "one serial progression");
}

#[tokio::test]
async fn catch_up_holds_the_publication_lock_at_commit() {
    let (_dir, layout, _scope) = stamped_layout();
    materialize_state(&layout).await.unwrap();
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let probe_layout = layout.clone();
    let probe_fired = fired.clone();
    crate::test_probes::arm("catch_up_before_commit", move || {
        probe_fired.set(true);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(probe_layout.publication_lock_path())?;
        anyhow::ensure!(
            fs2::FileExt::try_lock_exclusive(&file).is_err(),
            "the publication lock must be held while catch-up commits"
        );
        Ok(())
    });
    catch_up_state(&layout).await.unwrap();
    crate::test_probes::disarm("catch_up_before_commit");
    assert!(fired.get(), "the commit probe must have run");
}

#[tokio::test]
async fn a_writer_after_a_pre_head_fsync_crash_never_reuses_the_committed_serial() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_before_crash");

    crate::test_probes::crash_at("db_committed_before_head_fsync");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("db_committed_before_head_fsync");
    assert!(error.to_string().contains("injected crash"), "{error}");

    let pool = open_cache(&layout).await.unwrap();
    let (committed_serial, _, _) = stamped(&pool).await;
    pool.close().await;

    // The pass committed, but the crash landed before the post-commit head
    // fsync. A writer arriving now must still allocate past the committed
    // serial, or its event dies undrained when the next window opens.
    create_source(&store, &scope, "source_after_crash");
    let newest = events_in_window(&layout, 1, i64::MAX)
        .unwrap()
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap();
    assert!(
        newest > committed_serial,
        "event sequence {newest} must exceed the committed serial {committed_serial}"
    );

    let report = catch_up_state(&layout).await.unwrap();
    assert!(
        report.events_drained >= 1,
        "the writer's event must be drained, not pruned undrained: {report:?}"
    );
}

#[tokio::test]
async fn deleting_the_journal_directory_never_resets_the_sequence_space() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (committed_serial, _, _) = stamped(&pool).await;
    pool.close().await;

    let journal_dir = layout.cache_dir().join("journal");
    if journal_dir.exists() {
        std::fs::remove_dir_all(&journal_dir).unwrap();
    }

    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_after_journal_loss");
    let newest = events_in_window(&layout, 1, i64::MAX)
        .unwrap()
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap();
    assert!(
        newest > committed_serial,
        "event sequence {newest} must exceed the committed serial {committed_serial} \
         after the events directory is lost"
    );

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.events_drained >= 1, "{report:?}");
}
