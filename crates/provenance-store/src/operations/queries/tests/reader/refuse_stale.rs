use super::super::comparison::{served_stamped, test_stores};
use super::super::pinned::request_set;
use super::get_through;
use crate::cache::{catch_up_state, open_cache, unit_stored_digest, Unit};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader::ReadRefusal;
use provenance_core::protocol::StampPolicy;
use provenance_macros::verifies;
use test_stores::TestStore;

fn policy() -> ReadPolicy {
    ReadPolicy::with_freshness(FreshnessPolicy::RefuseStale)
}

fn edit(store: &TestStore) {
    let path = crate::shards::requirements_path(&store.layout(), &store.scope);
    // Invalid JSON must still produce a digest refusal, without a parse.
    std::fs::write(path, b"not JSON\n").unwrap();
}

fn replace_scope(store: &TestStore) {
    let path = store.layout().manifest_path();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["scopes"][0]["id"] = "new_scope".into();
    std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}

#[tokio::test]
#[verifies("rule_refuse_stale_writes_no_revision", examples)]
async fn refuse_stale_answers_at_the_stored_serial_when_every_unit_matches() {
    let store = test_stores::seeded_queries();
    let before = get_through(&store, ReadPolicy::default()).await.unwrap();
    crate::test_probes::start_recording_reads();
    let after = get_through(&store, policy()).await.unwrap();
    let reads = crate::test_probes::take_recorded_reads();
    assert!(reads.is_empty(), "the hash step parsed records: {reads:?}");
    assert_eq!(after.stamp.serial, before.stamp.serial);
    assert_eq!(after.stamp.digest, before.stamp.digest);
    assert_eq!(after.stamp.instance_id, before.stamp.instance_id);
    assert_eq!(after.stamp.policy, StampPolicy::RefuseStale);
    assert!(after.result.found);
}

#[tokio::test]
#[verifies("rule_refuse_stale_names_the_moved_units", examples)]
async fn refuse_stale_refuses_and_names_the_moved_unit() {
    let store = test_stores::seeded_queries();
    let before = get_through(&store, ReadPolicy::default()).await.unwrap();
    let unit = Unit::Scope(store.scope.clone());
    let stored = unit_stored_digest(&store.layout().state_dir(), &unit).unwrap();
    edit(&store);
    let live = unit_stored_digest(&store.layout().state_dir(), &unit).unwrap();
    let error = get_through(&store, policy()).await.unwrap_err();
    let Some(ReadRefusal::Stale {
        database,
        serial,
        digest,
        instance_id,
        moved,
    }) = error.downcast_ref::<ReadRefusal>()
    else {
        panic!("expected Stale, got {error:#}");
    };
    assert_eq!(database, &store.layout().cache_db_path());
    assert_eq!(*serial, before.stamp.serial);
    assert_eq!(digest, &before.stamp.digest);
    assert_eq!(instance_id, &before.stamp.instance_id);
    assert_eq!(
        moved,
        &[crate::operations::reader::MovedUnit {
            unit: unit.name(),
            stored: stored.clone(),
            live: live.clone(),
        }]
    );
    assert_eq!(error.to_string(), format!(
        "refuse_stale: the projection in {} at serial {} (digest {}, instance {}) is behind canonical state; moved: {} (stored {stored}, live {live}). Read under catch_up or run `provenance materialize`.",
        store.layout().cache_db_path(), before.stamp.serial, before.stamp.digest,
        before.stamp.instance_id, unit.name(),
    ));
}

#[tokio::test]
#[verifies("rule_refuse_stale_names_the_moved_units", examples)]
async fn refuse_stale_names_new_and_departed_scopes_in_order() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    replace_scope(&store);
    let error = get_through(&store, policy()).await.unwrap_err();
    let Some(ReadRefusal::Stale { moved, .. }) = error.downcast_ref::<ReadRefusal>() else {
        panic!("expected Stale, got {error:#}");
    };
    assert_eq!(
        moved
            .iter()
            .map(|unit| unit.unit.as_str())
            .collect::<Vec<_>>(),
        ["global", "scope:default", "scope:new_scope"]
    );
    assert!(moved[1].live.is_empty());
    assert!(moved[2].stored.is_empty());
    let text = error.to_string();
    assert!(text.contains("moved: global (stored sha256:"), "{text}");
    let departed = text.find("scope:default (stored sha256:").expect(&text);
    let new = text
        .find("scope:new_scope (stored , live sha256:")
        .expect(&text);
    assert!(departed < new, "{text}");
    assert!(text[..new].contains(", live )"), "{text}");
}

#[tokio::test]
#[verifies("rule_refuse_stale_writes_no_revision", examples)]
async fn refuse_stale_writes_no_revision_row() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let before = std::fs::read(store.layout().cache_db_path()).unwrap();
    get_through(&store, policy()).await.unwrap();
    assert_eq!(
        std::fs::read(store.layout().cache_db_path()).unwrap(),
        before
    );
    edit(&store);
    let error = get_through(&store, policy()).await.unwrap_err();
    assert!(error.to_string().contains("is behind canonical state"));
    assert_eq!(
        std::fs::read(store.layout().cache_db_path()).unwrap(),
        before
    );
}

#[tokio::test]
async fn refuse_stale_matches_annotate_only_bytes_over_the_pinned_store() {
    let store = TestStore::pinned();
    let base = store.base_commit.as_deref().expect("git on the path");
    catch_up_state(&store.layout()).await.unwrap();
    crate::test_probes::set_test_scan(None);
    for request in request_set(base) {
        let mut before = served_stamped(
            &store,
            &request,
            ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
        )
        .await
        .unwrap();
        let after = served_stamped(&store, &request, policy()).await.unwrap();
        before.stamp.policy = StampPolicy::RefuseStale;
        assert_eq!(
            serde_json::to_vec(&(after.result, after.stamp, after.freshness_error)).unwrap(),
            serde_json::to_vec(&(before.result, before.stamp, before.freshness_error)).unwrap(),
            "{} {}",
            request.operation(),
            request.describe()
        );
    }
}

#[tokio::test]
async fn refuse_stale_refuses_an_absent_projection() {
    let store = test_stores::seeded_queries();
    let error = get_through(&store, policy()).await.unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<ReadRefusal>(),
            Some(ReadRefusal::NoProjection { .. })
        ),
        "{error}"
    );
    assert!(!store.layout().cache_db_path().exists());
}

#[tokio::test]
async fn refuse_stale_refuses_a_projection_without_a_revision() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    sqlx::query("DELETE FROM projection_revision")
        .execute(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
    let error = get_through(&store, policy()).await.unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<ReadRefusal>(),
            Some(ReadRefusal::NoProjection { .. })
        ),
        "{error}"
    );
}

#[tokio::test]
async fn refuse_stale_refuses_a_half_migrated_projection() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let pool = open_cache(&store.layout()).await.unwrap();
    sqlx::query("DELETE FROM projection_family_digests")
        .execute(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
    let error = get_through(&store, policy()).await.unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<ReadRefusal>(),
            Some(ReadRefusal::HalfMigrated { .. })
        ),
        "{error}"
    );
}

#[tokio::test]
async fn refuse_stale_refuses_old_migrations_and_validation() {
    for sql in [
        "DELETE FROM _schema_migrations",
        "UPDATE projection_validation SET version = 0",
    ] {
        let store = test_stores::seeded_queries();
        catch_up_state(&store.layout()).await.unwrap();
        let pool = open_cache(&store.layout()).await.unwrap();
        sqlx::query(sql).execute(pool.pool()).await.unwrap();
        pool.close().await.unwrap();
        let error = get_through(&store, policy()).await.unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<ReadRefusal>(),
                Some(ReadRefusal::SchemaBehind { .. })
            ),
            "{error}"
        );
    }
}

#[tokio::test]
async fn refuse_stale_keeps_other_guard_errors() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let locks = store.layout().cache_dir().join("locks");
    std::fs::remove_dir_all(&locks).unwrap();
    std::fs::write(locks, b"").unwrap();
    let expected = crate::publication::publication_guard(&store.layout())
        .await
        .err()
        .unwrap();
    let error = get_through(&store, policy()).await.unwrap_err();
    assert_eq!(format!("{error:#}"), format!("{expected:#}"));
}

#[cfg(unix)]
#[tokio::test]
async fn refuse_stale_refuses_a_unit_it_cannot_hash() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let path = crate::shards::requirements_path(&store.layout(), &store.scope);
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("missing-shard", &path).unwrap();
    let error = get_through(&store, policy()).await.unwrap_err();
    let text = error.to_string();
    assert!(
        text.contains("scope:default") && text.contains(path.as_str()),
        "{text}"
    );
    let Some(ReadRefusal::UnitUnreadable {
        unit,
        path: unreadable,
        error: cause,
    }) = error.downcast_ref::<ReadRefusal>()
    else {
        panic!("expected UnitUnreadable, got {error:#}");
    };
    assert_eq!(unit, "scope:default");
    assert_eq!(unreadable, &path);
    assert!(!cause.is_empty());
    let answer = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert_eq!(answer.stamp.policy, StampPolicy::CatchUpFailed);
}

#[cfg(unix)]
#[tokio::test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
async fn refuse_stale_decides_when_the_lock_cannot_be_taken() {
    let store = test_stores::seeded_queries();
    let before = get_through(&store, ReadPolicy::default()).await.unwrap();
    let _restore = super::read_only::lock_untakeable(&store);
    let guard = crate::publication::publication_guard(&store.layout()).await;
    if guard.is_ok() {
        eprintln!("SKIPPED refuse_stale_decides_when_the_lock_cannot_be_taken: this user can open the protected lock");
        return;
    }
    let after = get_through(&store, policy()).await.unwrap();
    assert_eq!(after.stamp.serial, before.stamp.serial);
    assert_eq!(after.stamp.policy, StampPolicy::RefuseStale);
    let path = crate::shards::requirements_path(&store.layout(), &store.scope);
    let mut hashes = 0;
    crate::test_probes::arm("unit_files_collected", move || {
        hashes += 1;
        if hashes == 2 {
            std::fs::write(&path, b"changed during the unlocked hash\n")?;
        }
        Ok(())
    });
    let result = get_through(&store, policy()).await;
    crate::test_probes::disarm("unit_files_collected");
    let error = result.unwrap_err();
    assert!(
        matches!(error.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved.iter().any(|unit| unit.unit == "scope:default")),
        "{error}"
    );
    for suffix in ["wal", "shm"] {
        assert!(!store
            .layout()
            .cache_dir()
            .join(format!("provenance.db-{suffix}"))
            .exists());
    }
}

#[tokio::test]
async fn refuse_stale_keeps_the_checked_revision_after_a_publication() {
    let store = test_stores::seeded_queries();
    let before = get_through(&store, ReadPolicy::default()).await.unwrap();
    let layout = store.layout();
    let scope = store.scope.clone();
    crate::test_probes::arm("refuse_stale_after_hash", move || {
        assert!(!crate::test_probes::publication_lock_is_held(&layout));
        let path = crate::shards::requirements_path(&layout, &scope);
        crate::cache::tests::fixtures::rewrite_records(&path, |record| {
            record["statement"] = "Changed after the freshness check".into();
        });
        let layout = layout.clone();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(catch_up_state(&layout));
            let _ = send.send(result);
        });
        receive.recv_timeout(std::time::Duration::from_secs(10))??;
        Ok(())
    });
    let result = get_through(&store, policy()).await;
    crate::test_probes::disarm("refuse_stale_after_hash");
    let after = result.unwrap();
    assert_eq!(after.stamp.serial, before.stamp.serial);
    assert_eq!(after.stamp.digest, before.stamp.digest);
    assert_eq!(
        serde_json::to_value(after.result).unwrap(),
        serde_json::to_value(before.result).unwrap()
    );
    let latest = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap();
    assert_eq!(latest.stamp.serial, before.stamp.serial + 1);
}

#[tokio::test]
async fn refuse_stale_refuses_a_shard_that_vanishes_during_the_hash() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let path = crate::shards::requirements_path(&store.layout(), &store.scope);
    let removed = path.clone();
    let layout = store.layout();
    let mut hashes = 0;
    crate::test_probes::arm("unit_files_collected", move || {
        assert!(crate::test_probes::publication_lock_is_held(&layout));
        hashes += 1;
        if hashes == 2 {
            std::fs::remove_file(&removed)?;
        }
        Ok(())
    });
    let result = get_through(&store, policy()).await;
    crate::test_probes::disarm("unit_files_collected");
    let error = result.unwrap_err();
    assert!(error.to_string().contains(path.as_str()), "{error}");
    let Some(ReadRefusal::UnitUnreadable {
        unit,
        path: unreadable,
        error: cause,
    }) = error.downcast_ref::<ReadRefusal>()
    else {
        panic!("expected UnitUnreadable, got {error:#}");
    };
    assert_eq!(unit, "scope:default");
    assert_eq!(unreadable, &path);
    assert!(!cause.is_empty());
}
