//! The section lock of the local-disk adapter: a lock file under
//! `.provenance/cache/locks`, the checks of the directories it lives in, and
//! the recovery of an interrupted publication before the section runs.

use super::recovery::{
    canonical_transactions_dir, create_real_directory, recover_pending_publication,
};
use crate::layout::ProvenanceLayout;
use crate::publication::recover_pending_source_edit;
use crate::storage::SectionLock;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;
use std::fs::{File, OpenOptions};

/// The publication section of one layout.
///
/// An acquire refuses a `.provenance` directory that resolves outside the
/// repository and a symbolic link in place of a cache directory, takes the
/// exclusive lock on the publication lock file, and then completes or rolls
/// back a publication that a crash interrupted. Other processes take the
/// same lock file, so the section also excludes them.
#[derive(Clone, Debug)]
pub struct LocalSectionLock {
    layout: ProvenanceLayout,
}

impl LocalSectionLock {
    pub const fn new(layout: ProvenanceLayout) -> Self {
        Self { layout }
    }

    /// Takes the lock, runs `check` and then recovery. A failed check
    /// releases the lock and runs no recovery.
    pub fn acquire_checked(
        &self,
        check: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<LockedPublicationFile> {
        prepare_publication_lock(&self.layout)?;
        let lock = LockedPublicationFile::acquire(&self.layout.publication_lock_path())?;
        check()?;
        prepare_transaction_dirs(&self.layout)?;
        recover_pending_source_edit(&self.layout)?;
        recover_pending_publication(&self.layout)?;
        Ok(lock)
    }
}

impl SectionLock for LocalSectionLock {
    fn acquire(&self) -> anyhow::Result<Box<dyn Send>> {
        Ok(Box::new(self.acquire_checked(|| Ok(()))?))
    }
}

/// An advisory lock on an open publication lock file. Released on drop.
///
/// The lock belongs to the file description and not to a thread, so a
/// holder may move it to another thread.
pub struct LockedPublicationFile {
    file: File,
}

impl LockedPublicationFile {
    pub fn acquire(path: &Utf8Path) -> anyhow::Result<Self> {
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

/// Creates the directories of the lock file and refuses a `.provenance`
/// directory that resolves outside the repository.
pub fn prepare_publication_lock(layout: &ProvenanceLayout) -> anyhow::Result<()> {
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
pub fn canonical_utf8(path: &Utf8Path, description: &str) -> anyhow::Result<Utf8PathBuf> {
    let resolved =
        dunce::canonicalize(path).with_context(|| format!("resolve {description} {path}"))?;
    Utf8PathBuf::from_path_buf(resolved)
        .map_err(|path| anyhow::anyhow!("{description} is not UTF-8: {}", path.display()))
}

fn prepare_transaction_dirs(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    create_real_directory(&layout.import_transactions_dir())?;
    canonical_transactions_dir(layout)?;
    create_real_directory(&layout.source_edit_transactions_dir())
}
