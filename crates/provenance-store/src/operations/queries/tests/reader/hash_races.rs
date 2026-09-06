use super::super::comparison::test_stores;
use super::get_through;
use crate::cache::{catch_up_state, unit_digest, Unit};
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader::ReadRefusal;
use std::os::unix::fs::PermissionsExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

fn policy() -> ReadPolicy {
    ReadPolicy::with_freshness(FreshnessPolicy::RefuseStale)
}

#[derive(Default)]
struct PublicationAttempt {
    blocked: Arc<AtomicBool>,
    writer: Arc<Mutex<Option<std::thread::JoinHandle<anyhow::Result<()>>>>>,
}

impl PublicationAttempt {
    fn arm(
        layout: ProvenanceLayout,
        publish: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
    ) -> Self {
        let attempt = Self::default();
        let blocked = attempt.blocked.clone();
        let writer = attempt.writer.clone();
        let mut publish = Some(publish);
        let mut hashes = 0;
        crate::test_probes::arm("unit_files_collected", move || {
            hashes += 1;
            if hashes != 2 {
                return Ok(());
            }
            std::fs::set_permissions(
                layout.publication_lock_path(),
                std::fs::Permissions::from_mode(0o644),
            )?;
            std::fs::set_permissions(layout.cache_dir(), std::fs::Permissions::from_mode(0o755))?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(layout.publication_lock_path())?;
            let excluded = match fs2::FileExt::try_lock_exclusive(&file) {
                Ok(()) => {
                    fs2::FileExt::unlock(&file)?;
                    false
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => true,
                Err(error) => return Err(error.into()),
            };
            blocked.store(excluded, Ordering::SeqCst);
            let layout = layout.clone();
            let publish = publish.take().unwrap();
            let (send, receive) = std::sync::mpsc::channel();
            *writer.lock().unwrap() = Some(std::thread::spawn(move || {
                let result = crate::publication::with_repository_publication(&layout, publish);
                let _ = send.send(());
                result
            }));
            // On the old unlocked path, finish the publication before hashing resumes.
            if !excluded {
                receive.recv_timeout(std::time::Duration::from_secs(10))?;
            }
            Ok(())
        });
        attempt
    }

    fn finish(&self) {
        crate::test_probes::disarm("unit_files_collected");
        self.writer
            .lock()
            .unwrap()
            .take()
            .expect("writer attempted publication")
            .join()
            .unwrap()
            .unwrap();
    }
}

async fn publication_between_units(add_scope: bool) {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let shard = crate::shards::requirements_path(&store.layout(), &store.scope);
    let original = std::fs::read(&shard).unwrap();
    if !add_scope {
        crate::cache::tests::fixtures::rewrite_records(&shard, |record| {
            record["statement"] = "Changed before the hash".into();
        });
        let first = get_through(&store, policy()).await.unwrap_err();
        assert!(
            matches!(first.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved[0].unit == "scope:default")
        );
    }
    let _restore = super::read_only::lock_untakeable(&store);
    let error = crate::publication::publication_guard(&store.layout())
        .await
        .err();
    let Some(error) = error else {
        eprintln!("SKIPPED publication_between_units: lock permission fixture does not bind");
        return;
    };
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let layout = store.layout();
    let attempt = PublicationAttempt::arm(layout.clone(), move || {
        let path = layout.manifest_path();
        if add_scope {
            let mut manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
            let mut added = manifest["scopes"][0].clone();
            added["id"] = "new_scope".into();
            manifest["scopes"].as_array_mut().unwrap().push(added);
            std::fs::write(path, serde_json::to_vec(&manifest)?)?;
        } else {
            let mut bytes = std::fs::read(&path)?;
            bytes.push(b'\n');
            std::fs::write(path, bytes)?;
            std::fs::write(&shard, &original)?;
        }
        Ok(())
    });
    let result = get_through(&store, policy()).await;
    attempt.finish();
    let next = get_through(&store, policy()).await.unwrap_err();
    assert!(
        matches!(next.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved.iter().any(|unit| unit.unit == "global"))
    );
    assert!(
        !attempt.blocked.load(Ordering::SeqCst),
        "the permission fallback must test the completed publication without a lock"
    );
    let error =
        result.expect_err("the projection was stale at every instant, but the read answered");
    assert!(
        matches!(error.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved.iter().any(|unit| unit.unit == "global")),
        "{error:#}"
    );
    if add_scope {
        assert!(
            matches!(error.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved.iter().any(|unit| unit.unit == "scope:new_scope")),
            "{error:#}"
        );
    }
}

#[tokio::test]
async fn a_scope_added_between_unit_hashes_refuses() {
    publication_between_units(true).await;
}

#[tokio::test]
async fn a_scope_restored_after_the_global_hash_refuses() {
    publication_between_units(false).await;
}

#[tokio::test]
async fn a_guarded_read_refuses_a_file_added_after_collection() {
    let store = test_stores::seeded_queries();
    catch_up_state(&store.layout()).await.unwrap();
    let layout = store.layout();
    let scope = store.scope.clone();
    let before = unit_digest(&layout.state_dir(), &Unit::Scope(scope.clone())).unwrap();
    let mut hashes = 0;
    crate::test_probes::arm("unit_files_collected", move || {
        assert!(crate::test_probes::publication_lock_is_held(&layout));
        hashes += 1;
        if hashes == 2 {
            std::fs::write(
                layout
                    .state_dir()
                    .join("scopes")
                    .join(scope.as_str())
                    .join("added.txt"),
                b"new canonical bytes",
            )?;
        }
        Ok(())
    });
    let result = get_through(&store, policy()).await;
    crate::test_probes::disarm("unit_files_collected");
    let after = unit_digest(
        &store.layout().state_dir(),
        &Unit::Scope(store.scope.clone()),
    )
    .unwrap();
    eprintln!("stored={before}, live={after}, result={result:?}");
    assert_ne!(before, after);
    let error = result.expect_err("file added during hash was missed");
    assert!(
        matches!(error.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::UnitUnreadable { unit, path, .. }) if unit == "scope:default" && path.file_name() == Some("added.txt"))
    );
}

#[test]
fn unreadable_refusal_escapes_line_breaks_and_keeps_the_path() {
    let path = camino::Utf8PathBuf::from("repo\nname\r/state/file");
    let refusal = ReadRefusal::UnitUnreadable {
        unit: "global".into(),
        path: path.clone(),
        error: format!("cannot read {path}"),
    };
    let text = refusal.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(!text.contains('\r'));
    assert!(text.contains("repo\\nname\\r/state/file"));
    assert!(
        matches!(refusal, ReadRefusal::UnitUnreadable { path: original, .. } if original == path)
    );
}

#[tokio::test]
async fn a_read_only_hash_answers_without_a_readable_publication_lock() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let _restore = super::read_only::lock_untakeable(&store);
    let path = store.layout().publication_lock_path();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&path).is_ok() {
        eprintln!("SKIPPED a_read_only_hash_answers_without_a_readable_publication_lock: lock permissions do not bind");
        return;
    }
    let result = get_through(&store, policy()).await;
    assert!(result.unwrap().result.found);
}

#[tokio::test]
async fn a_read_only_hash_refuses_stale_state_with_a_pending_publication() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let transaction = store.layout().import_transactions_dir().join("pending");
    std::fs::create_dir(&transaction).unwrap();
    crate::publication::write_publication_marker(
        &store.layout(),
        &transaction,
        crate::publication::PublicationPhase::Prepared,
    )
    .unwrap();
    let shard = crate::shards::requirements_path(&store.layout(), &store.scope);
    crate::cache::tests::fixtures::rewrite_records(&shard, |record| {
        record["statement"] = "Changed before the read".into();
    });
    let _restore = super::read_only::lock_untakeable(&store);
    let error = crate::publication::publication_guard(&store.layout())
        .await
        .err()
        .unwrap();
    if error
        .downcast_ref::<std::io::Error>()
        .map(std::io::Error::kind)
        != Some(std::io::ErrorKind::PermissionDenied)
    {
        eprintln!(
            "SKIPPED a_read_only_hash_refuses_stale_state_with_a_pending_publication: lock permissions do not bind"
        );
        return;
    }
    let result = get_through(&store, policy()).await;
    let error = result.expect_err("hash answered from stale state with a pending publication");
    assert!(
        matches!(error.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved[0].unit == "scope:default"),
        "{error:#}"
    );
}

#[tokio::test]
async fn a_publication_after_the_unlocked_hash_keeps_the_checked_answer() {
    let store = test_stores::seeded_queries();
    let before = get_through(&store, ReadPolicy::default()).await.unwrap();
    let bytes = std::fs::read(store.layout().cache_db_path()).unwrap();
    let _restore = super::read_only::lock_untakeable(&store);
    let guard = crate::publication::publication_guard(&store.layout()).await;
    if guard.is_ok() {
        eprintln!("SKIPPED publication after unlocked hash: lock permissions do not bind");
        return;
    }
    assert_eq!(
        guard
            .err()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let layout = store.layout();
    let scope = store.scope.clone();
    crate::test_probes::arm("refuse_stale_after_hash", move || {
        std::fs::set_permissions(
            layout.publication_lock_path(),
            std::fs::Permissions::from_mode(0o644),
        )?;
        // The writer can publish canonical files. The database stays read-only.
        crate::publication::with_repository_publication(&layout, || {
            let path = crate::shards::requirements_path(&layout, &scope);
            crate::cache::tests::fixtures::rewrite_records(&path, |record| {
                record["statement"] = "Changed after the freshness check".into();
            });
            Ok(())
        })
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
    let error = get_through(&store, policy()).await.unwrap_err();
    assert!(
        matches!(error.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved[0].unit == "scope:default"),
        "{error:#}"
    );
    assert_eq!(
        std::fs::read(store.layout().cache_db_path()).unwrap(),
        bytes
    );
}
