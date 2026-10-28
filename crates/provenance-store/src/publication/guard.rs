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

use super::{
    prepare_publication_lock, prepare_transaction_dirs, read_only, recover_pending_publication,
    source_edit_recovery,
};
use crate::layout::ProvenanceLayout;
use crate::repository::Repository;
use anyhow::Context;
use camino::Utf8Path;
use fs2::FileExt;
use provenance_macros::rule;
use std::fs::{File, OpenOptions};

/// An advisory lock on an open publication lock file. Released on drop.
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
            .with_context(|| format!("open publication lock {path}"))?;
        file.lock_exclusive()
            .with_context(|| format!("acquire publication lock {path}"))?;
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
/// Under read-only validation the guard holds no lock. The private fields
/// restrict construction to this module.
pub struct PublicationGuard {
    _lock: Option<LockedPublicationFile>,
    repository: Repository,
}

impl PublicationGuard {
    /// Supplies the protected repository layout to guarded readers.
    #[rule("rule_guarded_reads_use_guard_repository")]
    pub(crate) fn layout(&self) -> &ProvenanceLayout {
        self.repository.layout()
    }

    /// The opened repository the guard protects.
    pub(crate) const fn repository(&self) -> &Repository {
        &self.repository
    }
}

/// Holds one synchronous publication section and supplies its guarded read view.
pub fn with_publication_guard<R>(
    repository: &Repository,
    operation: impl FnOnce(&PublicationGuard) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    super::with_repository_publication(repository.layout(), || {
        operation(&PublicationGuard {
            _lock: None,
            repository: repository.clone(),
        })
    })
}

/// Acquires the publication lock for an async holder.
///
/// The blocking wait runs on the blocking pool. Pending-publication recovery
/// runs before the caller sees the guard.
pub async fn publication_guard(repository: &Repository) -> anyhow::Result<PublicationGuard> {
    let key = super::publication_lock_identity(repository.layout())?;
    if read_only::active(&key) {
        return Ok(PublicationGuard {
            _lock: None,
            repository: repository.clone(),
        });
    }
    let repository = repository.clone();
    tokio::task::spawn_blocking(move || {
        let lock = {
            let layout = repository.layout();
            prepare_publication_lock(layout)?;
            let lock = LockedPublicationFile::acquire(&layout.publication_lock_path())?;
            prepare_transaction_dirs(layout)?;
            source_edit_recovery::recover_pending_source_edit(layout)?;
            recover_pending_publication(layout)?;
            lock
        };
        Ok(PublicationGuard {
            _lock: Some(lock),
            repository,
        })
    })
    .await
    .map_err(|error| anyhow::anyhow!("publication guard acquisition failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use crate::publication::{
        publication_guard, with_read_only_validation, with_repository_publication,
    };
    use crate::repository::Repository;
    use std::sync::mpsc;
    use std::time::Duration;

    fn repo() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repository = crate::cache::tests::fixtures::initialized(dir.path());
        (dir, repository)
    }

    #[tokio::test]
    async fn guard_excludes_the_synchronous_publication_path_until_dropped() {
        let (_dir, repository) = repo();
        let guard = publication_guard(&repository).await.unwrap();

        let (sender, receiver) = mpsc::channel();
        let thread_layout = repository.layout().clone();
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

    #[test]
    fn read_only_validation_bypasses_the_lock() {
        let (_dir, repository) = repo();
        let lock_path = repository.layout().publication_lock_path();
        // The open took the lock once; the read-only section must not take it again.
        std::fs::remove_file(&lock_path).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        with_read_only_validation(repository.layout(), || {
            let _guard = runtime.block_on(publication_guard(&repository))?;
            // No lock file means no lock was taken.
            assert!(!lock_path.exists());
            Ok(())
        })
        .unwrap();
    }
}
