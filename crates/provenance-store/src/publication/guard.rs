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
    prepare_import_transactions_dir, prepare_publication_lock, read_only,
    recover_pending_publication,
};
use crate::layout::ProvenanceLayout;
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
    layout: ProvenanceLayout,
}

impl PublicationGuard {
    /// Supplies the protected repository layout to guarded readers.
    #[rule("rule_guarded_reads_use_guard_repository")]
    pub(crate) const fn layout(&self) -> &ProvenanceLayout {
        &self.layout
    }
}

/// Acquires the publication lock for an async holder.
///
/// The blocking wait runs on the blocking pool. Pending-publication recovery
/// runs before the caller sees the guard.
pub async fn publication_guard(layout: &ProvenanceLayout) -> anyhow::Result<PublicationGuard> {
    let key = layout.publication_lock_path().to_string();
    if read_only::active(&key) {
        return Ok(PublicationGuard {
            _lock: None,
            layout: layout.clone(),
        });
    }
    let layout = layout.clone();
    tokio::task::spawn_blocking(move || {
        prepare_publication_lock(&layout)?;
        let lock = LockedPublicationFile::acquire(&layout.publication_lock_path())?;
        prepare_import_transactions_dir(&layout)?;
        recover_pending_publication(&layout)?;
        Ok(PublicationGuard {
            _lock: Some(lock),
            layout,
        })
    })
    .await
    .map_err(|error| anyhow::anyhow!("publication guard acquisition failed: {error}"))?
}

/// Holds a shared lock without writing the lock file or running recovery.
/// A pending publication must be recovered by a writer before this read.
pub async fn publication_read_guard(layout: &ProvenanceLayout) -> anyhow::Result<PublicationGuard> {
    let layout = layout.clone();
    tokio::task::spawn_blocking(move || {
        let path = layout.publication_lock_path();
        let file = File::open(&path)
            .with_context(|| format!("refuse_stale: open publication lock for reading {path}"))?;
        fs2::FileExt::lock_shared(&file)
            .with_context(|| format!("refuse_stale: acquire shared publication lock {path}"))?;
        let lock = LockedPublicationFile { file };
        anyhow::ensure!(
            !layout.publication_marker_path().try_exists()?,
            "refuse_stale: pending publication requires recovery; run `provenance materialize`"
        );
        Ok(PublicationGuard {
            _lock: Some(lock),
            layout,
        })
    })
    .await
    .map_err(|error| anyhow::anyhow!("publication read guard acquisition failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use crate::layout::ProvenanceLayout;
    use crate::publication::{
        publication_guard, with_read_only_validation, with_repository_publication,
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
