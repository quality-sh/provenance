use crate::layout::ProvenanceLayout;
use crate::storage::{Atomicity, WriteSet};
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use serde::{de::DeserializeOwned, Serialize};

mod durability;
mod guard;
mod holders;
mod read_only;
mod recovery;
mod source_edit;
mod source_edit_recovery;
mod staged;
mod write_permission;
pub use durability::{sync_directory, sync_tree};
pub(crate) use guard::SectionHandle;
pub use guard::{publication_guard, with_publication_guard, PublicationGuard};
pub use holders::PublicationHeld;
pub(crate) use holders::{enter, held_here, refuse_if_held, HolderKind};
pub use read_only::read_only_record_kinds;
pub(crate) use read_only::with_read_only_validation;
use recovery::{canonical_transactions_dir, create_real_directory};
pub use recovery::{
    clear_publication_marker, recover_pending_publication, write_publication_marker,
    PublicationPhase,
};
#[cfg(all(test, unix))]
use recovery::{validate_missing_transaction_dir, validated_transaction_dir};
#[cfg(any(unix, windows))]
pub use source_edit::{with_staged_state_and_source_edit, SourceEditRecoveryFailure};
pub use staged::LiveAccessDuringStage;
pub(crate) use staged::{
    refuse_held_access_during_stage, with_stage_body, with_staged_state, StageLock,
};
pub(crate) use write_permission::{protect_rows, WritePermissions};

/// Runs `operation` in the publication section of `layout`.
///
/// `operation` gets a store that holds the section. The store has no
/// repository identity. A second entry on a thread that holds the section
/// returns [`PublicationHeld`].
pub fn with_repository_publication<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce(&crate::state_store::StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    with_repository_publication_checked(
        layout,
        || Ok(()),
        || operation(&crate::state_store::StateStore::in_section(layout.clone())),
    )
}

/// Takes the publication lock of `layout`, runs `check` and recovery, then
/// `operation`. Only a store that holds the section may call into it again.
#[rule("rule_publication_section_is_not_entered_twice")]
pub(crate) fn with_repository_publication_checked<R>(
    layout: &ProvenanceLayout,
    check: impl FnOnce() -> anyhow::Result<()>,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let key = publication_lock_identity(layout)?;
    staged::refuse_during_stage(&key, layout.root())?;
    refuse_if_held(&key, HolderKind::Live, layout.root())?;
    prepare_publication_lock(layout)?;
    let _lock = guard::LockedPublicationFile::acquire(&layout.publication_lock_path())?;
    let _holder = enter(key, HolderKind::Live);
    check()?;
    prepare_transaction_dirs(layout)?;
    source_edit_recovery::recover_pending_source_edit(layout)?;
    recover_pending_publication(layout).and_then(|()| operation())
}

// Resolve the existing ancestor so planned-state validation creates no paths.
fn publication_lock_identity(layout: &ProvenanceLayout) -> anyhow::Result<String> {
    let absolute = std::path::absolute(layout.root())?;
    for ancestor in absolute.ancestors() {
        match dunce::canonicalize(ancestor) {
            Ok(resolved) => {
                let root = resolved.join(absolute.strip_prefix(ancestor)?);
                let root = Utf8PathBuf::from_path_buf(root).map_err(|path| {
                    anyhow::anyhow!("repository path is not UTF-8: {}", path.display())
                })?;
                return Ok(ProvenanceLayout::new(root)
                    .publication_lock_path()
                    .to_string());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("resolve repository path {}", layout.root()));
            }
        }
    }
    anyhow::bail!(
        "repository path has no existing ancestor: {}",
        layout.root()
    )
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
        dunce::canonicalize(path).with_context(|| format!("resolve {description} {path}"))?;
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
        self.mutate_jsonl_records_with_store(path, |_, records| mutate(records))
    }

    /// Like [`Self::mutate_jsonl_records`], but `mutate` also gets the store
    /// that holds the section, for the reads the change depends on.
    pub(crate) fn mutate_jsonl_records_with_store<T, R>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&Self, &mut Vec<T>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R>
    where
        T: DeserializeOwned + Serialize,
    {
        let mut result = None;
        self.publish_jsonl_if(path, |held, records| {
            result = Some(mutate(held, records)?);
            Ok(true)
        })?;
        Ok(result.expect("the mutation ran"))
    }

    /// Like [`Self::mutate_jsonl_records`], but writes the shard only when
    /// `mutate` returns `true`.
    pub(crate) fn mutate_jsonl_records_if<T>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool>
    where
        T: DeserializeOwned + Serialize,
    {
        self.publish_jsonl_if(path, |_, records| mutate(records))
    }

    /// Reads the shard at `path` once under its advisory lock, runs `mutate`,
    /// and publishes the new shard when `mutate` returns `true`. A change to
    /// an enrolled row needs a grant in the permissions of the store.
    fn publish_jsonl_if<T>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&Self, &mut Vec<T>) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool>
    where
        T: DeserializeOwned + Serialize,
    {
        self.ensure_writable()?;
        self.with_repository_publication(|held| {
            let lock_path = held.layout.state_shard_lock_path(path)?;
            crate::jsonl::with_advisory_lock(&lock_path, || {
                let contents = held.state_text(path)?;
                let bytes = crate::jsonl::mutate_jsonl_bytes(
                    path,
                    contents.as_deref(),
                    held.permissions(),
                    |records| mutate(held, records),
                )?;
                let Some(bytes) = bytes else {
                    return Ok(false);
                };
                held.publish_shard(path, bytes)?;
                Ok(true)
            })
        })
    }

    /// Publishes the shard at `path` as a one-key write set through the
    /// adapter of this store. The store holds the section, so the set
    /// carries no base revision.
    #[rule("rule_write_sets_publish_through_the_storage_port")]
    fn publish_shard(&self, path: &Utf8Path, bytes: Vec<u8>) -> anyhow::Result<()> {
        let writes = WriteSet::new(Atomicity::PerKey).put(self.state_key(path)?, bytes);
        self.storage()
            .publish(writes)
            .map(drop)
            .map_err(crate::write_error::publish_failure)
    }
}

#[cfg(all(test, unix))]
mod containment_tests;
#[cfg(test)]
mod tests;
