use crate::layout::ProvenanceLayout;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{de::DeserializeOwned, Serialize};
use std::cell::RefCell;
use std::collections::BTreeSet;

mod durability;
mod guard;
mod read_only;
mod recovery;
mod source_edit;
mod staged;
pub use durability::{sync_directory, sync_tree};
pub use guard::{publication_guard, PublicationGuard};
pub use read_only::with_read_only_validation;
pub use recovery::{
    clear_publication_marker, recover_pending_publication, write_publication_marker,
    PublicationPhase,
};
pub use staged::with_staged_state;
#[cfg(any(unix, windows))]
pub use source_edit::{with_staged_state_and_source_edit, SourceEditRecoveryFailure};
use recovery::{canonical_transactions_dir, create_real_directory};
#[cfg(test)]
use recovery::{validate_missing_transaction_dir, validated_transaction_dir};

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

pub fn with_repository_publication<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    with_repository_publication_checked(layout, || Ok(()), operation)
}

pub(crate) fn with_repository_publication_checked<R>(
    layout: &ProvenanceLayout,
    check: impl FnOnce() -> anyhow::Result<()>,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let key = layout.publication_lock_path().to_string();
    if read_only::active(&key) {
        check()?;
        return operation();
    }
    prepare_publication_lock(layout)?;
    let lock_path = layout.publication_lock_path();
    let key = lock_path.to_string();
    if HELD_LOCKS.with(|locks| locks.borrow().contains(&key)) {
        check()?;
        return operation();
    }
    let _lock = guard::LockedPublicationFile::acquire(&lock_path)?;
    let _held_lock = HeldPublicationLock::new(key);
    check()?;
    prepare_transaction_dirs(layout)?;
    source_edit::recover_pending_source_edit(layout)?;
    recover_pending_publication(layout).and_then(|()| operation())
}

fn prepare_publication_lock(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let canonical_root = canonical_utf8(
        layout
            .provenance_dir()
            .parent()
            .unwrap_or_else(|| Utf8Path::new(".")),
        "repository path",
    )?;
    let provenance = layout.provenance_dir();
    create_real_directory(&provenance)?;
    let canonical_provenance = canonical_utf8(&provenance, "provenance path")?;
    anyhow::ensure!(
        canonical_provenance == canonical_root.join(".provenance"),
        "repository provenance directory is outside the repository"
    );

    let cache = layout.cache_dir();
    create_real_directory(&cache)?;
    let locks = cache.join("locks");
    create_real_directory(&locks)?;
    Ok(())
}

/// Resolves `path`, keeping the failure message that names what the path was.
fn canonical_utf8(path: &Utf8Path, description: &str) -> anyhow::Result<Utf8PathBuf> {
    let resolved =
        std::fs::canonicalize(path).with_context(|| format!("resolve {description} {path}"))?;
    Utf8PathBuf::from_path_buf(resolved)
        .map_err(|path| anyhow::anyhow!("{description} is not UTF-8: {}", path.display()))
}

fn prepare_transaction_dirs(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    create_real_directory(&layout.import_transactions_dir())?;
    canonical_transactions_dir(layout)?;
    create_real_directory(&layout.source_edit_transactions_dir())
}

impl crate::state_store::StateStore {
    pub(crate) fn mutate_jsonl_records<T, R>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R>
    where
        T: DeserializeOwned + Serialize,
    {
        self.with_repository_publication(|| {
            let lock_path = self.layout.state_shard_lock_path(path)?;
            crate::jsonl::mutate_jsonl_locked(path, &lock_path, mutate)
        })
    }
}

#[cfg(all(test, unix))]
mod containment_tests;
#[cfg(test)]
mod tests;
#[cfg(all(test, any(unix, windows)))]
mod source_edit_recovery_tests;
