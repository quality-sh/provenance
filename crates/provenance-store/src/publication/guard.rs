//! The owned publication guard.
//!
//! One `PublicationGuard` is the only holder record for the async path:
//! the async materializer acquires it once and passes an explicit
//! `&PublicationGuard` capability to locked helper variants, so a helper
//! that needs the lock held cannot be called without one. The lock lives
//! in the open file description the guard owns, so it is held across
//! `.await` points without any thread identity assumptions. Helpers under
//! the async path never reacquire the publication lock. No `block_on`, no
//! bridge thread, no task or thread registry, no nested async
//! reacquisition.
//!
//! The synchronous `with_repository_publication` entry delegates
//! acquisition, recovery, and release to the same helpers over the same
//! low-level lock primitive, and keeps its synchronous nesting reuse for
//! callers that nest on one thread.

use super::{
    prepare_import_transactions_dir, prepare_publication_lock, recover_pending_publication,
    StateSnapshot,
};
use crate::layout::ProvenanceLayout;
use camino::Utf8Path;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::File;

thread_local! {
    static HELD_LOCKS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

struct HeldPublicationLock {
    key: String,
}

impl HeldPublicationLock {
    fn new(key: String) -> Self {
        HELD_LOCKS.with(|locks| locks.borrow_mut().insert(key.clone()));
        Self { key }
    }
}

impl Drop for HeldPublicationLock {
    fn drop(&mut self) {
        HELD_LOCKS.with(|locks| locks.borrow_mut().remove(&self.key));
    }
}

/// The one low-level publication lock primitive: an exclusive advisory
/// file lock on the open file description of the lock file.
pub(super) struct ExclusiveFileLock {
    file: File,
}

impl ExclusiveFileLock {
    pub(super) fn acquire(path: &Utf8Path) -> anyhow::Result<Self> {
        use fs2::FileExt as _;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::options()
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

impl Drop for ExclusiveFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// An owned publication section. Dropping the guard releases the lock.
pub struct PublicationGuard {
    lock: Option<ExclusiveFileLock>,
}

impl PublicationGuard {
    /// Whether this guard owns the publication file lock. A guard taken
    /// under the read-only bypass holds nothing.
    pub const fn holds_file_lock(&self) -> bool {
        self.lock.is_some()
    }

    const fn from_lock(lock: Option<ExclusiveFileLock>) -> Self {
        Self { lock }
    }
}

/// Acquires the repository publication guard.
///
/// The blocking lock wait runs on the runtime's blocking pool; the
/// returned guard owns the open lock file and keeps the lock held across
/// migrations, the `SQLite` transaction, and commit, releasing it on drop.
pub async fn publication_guard(layout: &ProvenanceLayout) -> anyhow::Result<PublicationGuard> {
    let key = layout.publication_lock_path().to_string();
    if super::read_only::active(&key) {
        return Ok(PublicationGuard::from_lock(None));
    }
    prepare_publication_lock(layout)?;
    let blocking_layout = layout.clone();
    let lock = tokio::task::spawn_blocking(move || {
        ExclusiveFileLock::acquire(&blocking_layout.publication_lock_path())
    })
    .await
    .map_err(|error| anyhow::anyhow!("publication lock task failed: {error}"))??;
    prepare_import_transactions_dir(layout)?;
    recover_pending_publication(layout)?;
    Ok(PublicationGuard::from_lock(Some(lock)))
}

/// The synchronous publication section over the same primitives.
///
/// A nested call on one thread reuses the caller's held section, matching
/// the historical `HELD_LOCKS` behavior exactly.
pub(super) fn synchronous_publication<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let key = layout.publication_lock_path().to_string();
    if super::read_only::active(&key) {
        return operation();
    }
    prepare_publication_lock(layout)?;
    let lock_path = layout.publication_lock_path();
    let key = lock_path.to_string();
    if HELD_LOCKS.with(|locks| locks.borrow().contains(&key)) {
        return operation();
    }
    let lock = ExclusiveFileLock::acquire(&lock_path)?;
    let held_lock = HeldPublicationLock::new(key);
    prepare_import_transactions_dir(layout)?;
    let result = recover_pending_publication(layout).and_then(|()| operation());
    drop(held_lock);
    drop(lock);
    result
}

/// Copies canonical state into a snapshot, under a guard the caller
/// already holds.
pub fn snapshot_state_under_guard(
    guard: &PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<StateSnapshot> {
    debug_assert!(
        guard.holds_file_lock() || lock_is_nested(layout),
        "snapshot_state_under_guard requires a held publication guard"
    );
    snapshot_body(layout)
}

fn lock_is_nested(layout: &ProvenanceLayout) -> bool {
    HELD_LOCKS.with(|locks| {
        locks
            .borrow()
            .contains(layout.publication_lock_path().to_string().as_str())
    })
}

/// Test accessor for the synchronous nesting registry.
#[cfg(test)]
pub(super) fn nested_lock_held(key: &str) -> bool {
    HELD_LOCKS.with(|locks| locks.borrow().contains(key))
}

/// The shared snapshot body behind the synchronous and guarded entries.
pub(super) fn snapshot_body(layout: &ProvenanceLayout) -> anyhow::Result<StateSnapshot> {
    let directory = tempfile::tempdir()?;
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("snapshot path is not UTF-8: {}", path.display()))?;
    let snapshot_layout = ProvenanceLayout::new(root);
    copy_tree(&layout.state_dir(), &snapshot_layout.state_dir())?;
    Ok(StateSnapshot {
        _directory: directory,
        layout: snapshot_layout,
    })
}

use camino::Utf8PathBuf;

fn copy_tree(source: &Utf8Path, destination: &Utf8Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_child = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow::anyhow!("state path is not UTF-8: {}", path.display()))?;
        let destination_child = destination.join(entry.file_name().to_string_lossy().as_ref());
        let file_type = std::fs::symlink_metadata(&source_child)?.file_type();
        if file_type.is_dir() {
            copy_tree(&source_child, &destination_child)?;
        } else if file_type.is_file() {
            std::fs::copy(source_child, destination_child)?;
        } else {
            anyhow::bail!("unsupported state entry: {source_child}");
        }
    }
    Ok(())
}
