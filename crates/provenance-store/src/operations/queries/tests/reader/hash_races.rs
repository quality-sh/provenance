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
    let before = get_through(&store, ReadPolicy::default()).await.unwrap();
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
    if add_scope {
        assert!(
            attempt.blocked.load(Ordering::SeqCst),
            "publication during unlocked hash returned an answer"
        );
        let after = result.unwrap();
        assert_eq!(after.stamp.serial, before.stamp.serial);
        assert_eq!(after.stamp.digest, before.stamp.digest);
        assert_eq!(
            serde_json::to_value(after.result).unwrap(),
            serde_json::to_value(before.result).unwrap()
        );
        assert!(
            matches!(next.downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved.iter().any(|unit| unit.unit == "scope:new_scope"))
        );
    } else {
        assert!(
            result.is_err(),
            "the projection was stale at every instant, but the read answered"
        );
        assert!(
            matches!(result.unwrap_err().downcast_ref::<ReadRefusal>(), Some(ReadRefusal::Stale { moved, .. }) if moved[0].unit == "scope:default")
        );
        assert!(attempt.blocked.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn publication_cannot_add_a_scope_between_unit_hashes() {
    publication_between_units(true).await;
}

#[tokio::test]
async fn publication_cannot_restore_a_scope_after_the_global_hash() {
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
async fn a_read_only_hash_refuses_without_a_readable_publication_lock() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let _restore = super::read_only::lock_untakeable(&store);
    let path = store.layout().publication_lock_path();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&path).is_ok() {
        eprintln!("SKIPPED a_read_only_hash_refuses_without_a_readable_publication_lock: lock permissions do not bind");
        return;
    }
    let result = get_through(&store, policy()).await;
    assert!(result.is_err(), "hash answered without a publication lock");
}

#[tokio::test]
async fn a_read_only_hash_refuses_a_pending_publication() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    std::fs::write(store.layout().publication_marker_path(), b"{}").unwrap();
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
            "SKIPPED a_read_only_hash_refuses_a_pending_publication: lock permissions do not bind"
        );
        return;
    }
    let result = get_through(&store, policy()).await;
    let error = result.expect_err("hash answered during a pending publication");
    assert!(
        error.to_string().contains("pending publication"),
        "{error:#}"
    );
}
