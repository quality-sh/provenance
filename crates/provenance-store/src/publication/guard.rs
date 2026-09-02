//! The owned publication guard.
//!
//! The guard owns the open lock file. The lock belongs to that file
//! description and not to a thread, so a holder may await while it is held.
//! Helpers that need the lock take `&PublicationGuard`.
//!
//! Constraint: a synchronous `with_repository_publication` section entered
//! on a runtime worker thread blocks that worker in `flock` while a guard is
//! held elsewhere. Enough such callers starve the runtime, and the guard
//! holder can never run to release the lock. The one-command CLI cannot
//! reach this. A served process must move synchronous publication sections
//! to `spawn_blocking` or make them async first.
//!
//! Readers called under a held guard use the snapshot layout. Their own
//! lock sections take the snapshot's lock path, not the repository lock.

use super::{
    prepare_import_transactions_dir, prepare_publication_lock, read_only,
    recover_pending_publication, snapshot_state_unlocked, StateSnapshot,
};
use crate::layout::ProvenanceLayout;
use camino::Utf8Path;
use fs2::FileExt;
use std::fs::{File, OpenOptions};

/// An exclusive advisory lock on an open publication lock file. Released on
/// drop.
pub(super) struct LockedPublicationFile {
    file: File,
}

impl LockedPublicationFile {
    pub(super) fn acquire(path: &Utf8Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| anyhow::anyhow!("open publication lock {path}: {error}"))?;
        file.lock_exclusive()
            .map_err(|error| anyhow::anyhow!("acquire publication lock {path}: {error}"))?;
        Ok(Self { file })
    }
}

impl Drop for LockedPublicationFile {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

/// A held publication lock.
///
/// Under read-only validation the guard holds no lock. The private field
/// means only [`publication_guard`] can build one.
pub struct PublicationGuard {
    _lock: Option<LockedPublicationFile>,
}

/// Acquires the publication lock for an async holder.
///
/// The blocking wait runs on the blocking pool. Pending-publication recovery
/// runs before the caller sees the guard.
pub async fn publication_guard(layout: &ProvenanceLayout) -> anyhow::Result<PublicationGuard> {
    let key = layout.publication_lock_path().to_string();
    if read_only::active(&key) {
        return Ok(PublicationGuard { _lock: None });
    }
    let layout = layout.clone();
    tokio::task::spawn_blocking(move || {
        prepare_publication_lock(&layout)?;
        let lock = LockedPublicationFile::acquire(&layout.publication_lock_path())?;
        prepare_import_transactions_dir(&layout)?;
        recover_pending_publication(&layout)?;
        Ok(PublicationGuard { _lock: Some(lock) })
    })
    .await
    .map_err(|error| anyhow::anyhow!("publication guard acquisition failed: {error}"))?
}

/// Copies the state tree for a caller that holds the guard. Takes no lock.
///
/// ```compile_fail
/// use provenance_store::layout::ProvenanceLayout;
/// use provenance_store::publication::{snapshot_state_under_guard, PublicationGuard};
/// let layout = ProvenanceLayout::new("repo");
/// let forged = PublicationGuard { _lock: None };
/// let _ = snapshot_state_under_guard(&forged, &layout);
/// ```
pub fn snapshot_state_under_guard(
    _guard: &PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<StateSnapshot> {
    snapshot_state_unlocked(layout)
}

#[cfg(test)]
mod tests {
    use crate::layout::ProvenanceLayout;
    use crate::publication::{
        publication_guard, snapshot_state_under_guard, with_read_only_validation,
        with_repository_publication,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    fn repo_layout() -> (tempfile::TempDir, ProvenanceLayout) {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(root);
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        (dir, layout)
    }

    #[tokio::test]
    async fn guard_excludes_the_synchronous_publication_path_until_dropped() {
        let (_dir, layout) = repo_layout();
        let guard = publication_guard(&layout).await.unwrap();

        let (sender, receiver) = mpsc::channel();
        let thread_layout = layout.clone();
        let waiter = std::thread::spawn(move || {
            with_repository_publication(&thread_layout, || {
                sender.send(()).unwrap();
                Ok(())
            })
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(300)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "a canonical write must wait while the guard is held"
        );

        drop(guard);
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("the waiting write must proceed after the guard drops");
        waiter.join().unwrap().unwrap();
    }

    #[tokio::test]
    async fn snapshot_under_guard_takes_no_second_lock() {
        let (_dir, layout) = repo_layout();
        std::fs::write(layout.state_dir().join("probe.json"), b"{}").unwrap();
        let guard = publication_guard(&layout).await.unwrap();
        let snapshot = snapshot_state_under_guard(&guard, &layout).unwrap();
        assert!(snapshot.layout().state_dir().join("probe.json").exists());
    }

    #[test]
    fn read_only_validation_bypasses_the_lock() {
        let (_dir, layout) = repo_layout();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        with_read_only_validation(&layout, || {
            let _guard = runtime.block_on(publication_guard(&layout))?;
            // No lock file means no lock was taken.
            assert!(!layout.publication_lock_path().exists());
            Ok(())
        })
        .unwrap();
    }
}
