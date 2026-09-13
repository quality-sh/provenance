use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_core::protocol::StampPolicy;
use provenance_macros::verifies;
use std::os::unix::fs::PermissionsExt;

pub(super) struct Restore(Vec<(camino::Utf8PathBuf, std::fs::Permissions)>);

impl Restore {
    fn set(&mut self, path: camino::Utf8PathBuf, mode: u32) {
        let old = std::fs::metadata(&path).unwrap().permissions();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        self.0.push((path, old));
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        for (path, permissions) in self.0.drain(..).rev() {
            std::fs::set_permissions(path, permissions).unwrap();
        }
    }
}

pub(super) fn lock_untakeable(store: &TestStore) -> Restore {
    let mut restore = Restore(Vec::new());
    restore.set(store.layout().publication_lock_path(), 0o444);
    restore.set(store.layout().cache_dir(), 0o555);
    restore
}

async fn read_only_answer(untakeable: bool) {
    let store = test_stores::seeded_queries();
    let healthy = get_through(&store, ReadPolicy::default()).await.unwrap();
    let mut restore = if untakeable {
        lock_untakeable(&store)
    } else {
        Restore(Vec::new())
    };
    if !untakeable {
        restore.set(store.layout().cache_dir(), 0o555);
    }
    let probe = store.layout().cache_dir().join("probe");
    if std::fs::write(&probe, b"").is_ok() {
        std::fs::remove_file(probe).unwrap();
        eprintln!(
            "SKIPPED read_only_answer: this user writes through mode 0o555, so the fixture cannot \
             make the directory unwritable"
        );
        return;
    }
    if untakeable {
        let error = crate::publication::publication_guard(&store.layout())
            .await
            .err()
            .unwrap();
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::PermissionDenied),
            "{error:#}"
        );
    }
    for policy in [FreshnessPolicy::AnnotateOnly, FreshnessPolicy::CatchUp] {
        let answer = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap();
        assert!(answer.result.found);
        assert_eq!(answer.stamp.serial, healthy.stamp.serial);
        assert_eq!(answer.stamp.digest, healthy.stamp.digest);
        assert_eq!(
            answer.stamp.policy,
            if policy == FreshnessPolicy::CatchUp {
                StampPolicy::CatchUpFailed
            } else {
                StampPolicy::AnnotateOnly
            }
        );
        assert_eq!(
            answer.freshness_error.is_some(),
            policy == FreshnessPolicy::CatchUp
        );
        assert!(!store
            .layout()
            .cache_dir()
            .join("provenance.db-wal")
            .exists());
        assert!(!store
            .layout()
            .cache_dir()
            .join("provenance.db-shm")
            .exists());
    }
}

#[tokio::test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
async fn annotate_only_on_a_read_only_checkout_answers_at_its_serial() {
    read_only_answer(false).await;
}

#[tokio::test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
async fn annotate_only_answers_when_the_lock_cannot_be_taken() {
    read_only_answer(true).await;
}

#[tokio::test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
async fn a_half_migrated_projection_refuses_without_taking_the_lock() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let pool = crate::cache::open_cache(&store.layout()).await.unwrap();
    sqlx::query("DELETE FROM projection_family_digests")
        .execute(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
    let _restore = lock_untakeable(&store);
    let error = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<crate::operations::reader::ReadRefusal>(),
            Some(crate::operations::reader::ReadRefusal::HalfMigrated { .. })
        ),
        "{error:#}"
    );
}

#[tokio::test]
async fn a_guard_directory_permission_failure_keeps_its_io_kind() {
    let dir = tempfile::tempdir().unwrap();
    let layout = crate::layout::ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir(layout.provenance_dir()).unwrap();
    let mut restore = Restore(Vec::new());
    restore.set(layout.provenance_dir(), 0o555);
    let probe = layout.provenance_dir().join("probe");
    if std::fs::write(&probe, b"").is_ok() {
        std::fs::remove_file(probe).unwrap();
        eprintln!(
            "SKIPPED a_guard_directory_permission_failure_keeps_its_io_kind: this user writes \
             through mode 0o555, so the fixture cannot make the directory unwritable"
        );
        return;
    }
    let error = crate::publication::publication_guard(&layout)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::PermissionDenied),
        "{error:#}"
    );
}
