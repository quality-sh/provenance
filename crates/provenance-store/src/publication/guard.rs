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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

    /// The opened repository the guard protects. Under a synchronous section
    /// its stores hold that section: they read and write, stage and take a
    /// guard with no second lock. After the section ends, each state access
    /// through it or its clones refuses.
    pub const fn repository(&self) -> &Repository {
        &self.repository
    }
}

/// The life of one synchronous section, shared by the repository handles it
/// gives out.
#[derive(Clone, Debug)]
pub(crate) struct SectionHandle {
    open: Arc<AtomicBool>,
}

impl SectionHandle {
    /// Refuses after the section ended.
    #[rule("rule_section_repository_stops_with_its_section")]
    pub(crate) fn ensure_open(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.open.load(Ordering::Acquire),
            "the publication section of this repository handle has ended"
        );
        Ok(())
    }
}

/// Closes a section handle when the section ends, also on unwind.
struct SectionClose(SectionHandle);

impl Drop for SectionClose {
    fn drop(&mut self) {
        self.0.open.store(false, Ordering::Release);
    }
}

/// Holds one synchronous publication section and supplies its guarded read
/// view. The guard's repository holds the section. A repository that already
/// holds a section runs `operation` in that section.
pub fn with_publication_guard<R>(
    repository: &Repository,
    operation: impl FnOnce(&PublicationGuard) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    if let Some(section) = repository.section() {
        section.ensure_open()?;
        return operation(&PublicationGuard {
            _lock: None,
            repository: repository.clone(),
        });
    }
    super::with_repository_publication(repository.layout(), |_| {
        let section = SectionHandle {
            open: Arc::new(AtomicBool::new(true)),
        };
        let _close = SectionClose(section.clone());
        operation(&PublicationGuard {
            _lock: None,
            repository: repository.within_section(section),
        })
    })
}

/// Acquires the publication lock for an async holder.
///
/// The blocking wait runs on the blocking pool. Pending-publication recovery
/// runs before the caller sees the guard. The guard adds no holder entry: it
/// can move to another thread, and a synchronous section on any thread then
/// waits for it.
pub async fn publication_guard(repository: &Repository) -> anyhow::Result<PublicationGuard> {
    let key = super::publication_lock_identity(repository.layout())?;
    // The lock waits on the blocking pool, so a stage body that blocks on this guard would
    // wait on its own publication. The refusal runs here, on the calling thread.
    super::staged::refuse_during_stage(&key)?;
    if let Some(section) = repository.section() {
        section.ensure_open()?;
        return Ok(PublicationGuard {
            _lock: None,
            repository: repository.clone(),
        });
    }
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
        publication_guard, with_publication_guard, with_read_only_validation,
        with_repository_publication,
    };
    use crate::repository::Repository;
    use provenance_macros::verifies;
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
            with_repository_publication(&thread_layout, |_| {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[verifies("rule_publication_holders_grant_nothing", examples)]
    async fn a_guard_dropped_on_another_worker_leaves_no_holder() {
        let (_dir, repository) = repo();
        let guard = publication_guard(&repository).await.unwrap();
        let taken_on = std::thread::current().id();
        let layout = repository.layout().clone();
        let dropped_on = tokio::task::spawn_blocking(move || {
            drop(guard);
            with_repository_publication(&layout, |_| Ok(()))
                .expect("a section on the thread that dropped the guard proceeds");
            std::thread::current().id()
        })
        .await
        .unwrap();
        assert_ne!(taken_on, dropped_on);
        with_repository_publication(repository.layout(), |_| Ok(()))
            .expect("a section on the thread that took the guard proceeds");
    }

    #[test]
    #[verifies("rule_publication_section_is_not_entered_twice", examples)]
    fn the_guard_repository_stages_and_reads_in_the_section() {
        let (_dir, repository) = repo();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let manifest = with_publication_guard(&repository, |guard| {
            let held = guard.repository();
            held.stage(false, None, |staged| staged.manifest())?;
            with_publication_guard(held, |inner| inner.repository().manifest())?;
            drop(runtime.block_on(publication_guard(held))?);
            held.store().manifest()
        })
        .unwrap();
        assert_eq!(manifest, repository.manifest().unwrap());
    }

    #[test]
    #[verifies("rule_section_repository_stops_with_its_section", examples)]
    fn a_guard_repository_kept_past_its_section_is_refused() {
        let (_dir, repository) = repo();
        let kept =
            with_publication_guard(&repository, |guard| Ok(guard.repository().clone())).unwrap();
        let lock_path = repository.layout().publication_lock_path();
        std::fs::remove_file(&lock_path).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        assert!(kept.store().list_scope_directories().is_err());
        assert!(kept.manifest().is_err());
        assert!(kept.stage(false, None, |_| Ok(())).is_err());
        assert!(with_publication_guard(&kept, |_| Ok(())).is_err());
        assert!(runtime.block_on(publication_guard(&kept)).is_err());
        // No lock file means no refused access took the lock.
        assert!(!lock_path.exists());
        repository
            .store()
            .list_scope_directories()
            .expect("the handle that opened the repository still reads");
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
