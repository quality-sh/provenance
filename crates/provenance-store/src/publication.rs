use crate::layout::ProvenanceLayout;
#[cfg(feature = "host")]
use crate::storage::local::LocalSectionLock;
use crate::storage::{Atomicity, SectionLock, WriteSet};
#[cfg(feature = "host")]
use anyhow::Context;
use camino::Utf8Path;
#[cfg(feature = "host")]
use camino::Utf8PathBuf;
use provenance_macros::rule;
use serde::{de::DeserializeOwned, Serialize};

mod guard;
mod holders;
#[cfg(feature = "host")]
mod read_only;
#[cfg(feature = "host")]
mod source_edit;
#[cfg(feature = "host")]
mod source_edit_recovery;
mod staged;
mod write_permission;
#[cfg(feature = "host")]
pub use crate::storage::local::durability::{sync_directory, sync_tree};
#[cfg(feature = "host")]
pub use crate::storage::local::{
    clear_publication_marker, recover_pending_publication, write_publication_marker,
    PublicationPhase,
};
#[cfg(all(test, unix))]
use crate::storage::local::{validate_missing_transaction_dir, validated_transaction_dir};
#[cfg(feature = "host")]
pub use guard::publication_guard;
pub(crate) use guard::SectionHandle;
pub use guard::{with_publication_guard, PublicationGuard};
pub use holders::PublicationHeld;
pub(crate) use holders::{enter, held_here, refuse_if_held, HolderKind};
#[cfg(feature = "host")]
pub use read_only::read_only_record_kinds;
#[cfg(feature = "host")]
pub(crate) use read_only::with_read_only_validation;
#[cfg(all(feature = "host", any(unix, windows)))]
pub use source_edit::{with_staged_state_and_source_edit, SourceEditRecoveryFailure};
#[cfg(feature = "host")]
pub(crate) use source_edit_recovery::recover_pending_source_edit;
#[cfg(feature = "host")]
pub(crate) use staged::with_stage_body;
pub use staged::LiveAccessDuringStage;
pub(crate) use staged::{refuse_held_access_during_stage, with_staged_state, StageLock};
pub(crate) use write_permission::{protect_rows, WritePermissions};

/// Runs `operation` in the publication section of `layout`.
///
/// `operation` gets a store that holds the section. The store has no
/// repository identity. A second entry on a thread that holds the section
/// returns [`PublicationHeld`].
#[cfg(feature = "host")]
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
#[cfg(feature = "host")]
pub(crate) fn with_repository_publication_checked<R>(
    layout: &ProvenanceLayout,
    check: impl FnOnce() -> anyhow::Result<()>,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    enter_section(
        layout,
        || {
            Ok(Box::new(
                LocalSectionLock::new(layout.clone()).acquire_checked(check)?,
            ))
        },
        operation,
    )
}

/// Takes the section `lock` of the repository at `layout`, then runs
/// `operation`. Only a store that holds the section may call into it again.
pub(crate) fn with_section<R>(
    layout: &ProvenanceLayout,
    lock: &dyn SectionLock,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    enter_section(layout, || lock.acquire(), operation)
}

/// Refuses a second entry on this thread and live access in a stage body,
/// holds the section from `acquire`, then runs `operation`.
#[rule("rule_publication_section_is_not_entered_twice")]
fn enter_section<R>(
    layout: &ProvenanceLayout,
    acquire: impl FnOnce() -> anyhow::Result<Box<dyn Send>>,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let key = publication_lock_identity(layout)?;
    staged::refuse_during_stage(&key, layout.root())?;
    refuse_if_held(&key, HolderKind::Live, layout.root())?;
    let _section = acquire()?;
    let _holder = enter(key, HolderKind::Live);
    operation()
}

// Resolve the existing ancestor so planned-state validation creates no paths.
#[cfg(feature = "host")]
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

/// The key of the section of `layout`. With no file system to resolve the
/// root against, the key is the lock path of the layout as given.
#[cfg(not(feature = "host"))]
#[allow(clippy::unnecessary_wraps)]
fn publication_lock_identity(layout: &ProvenanceLayout) -> anyhow::Result<String> {
    Ok(layout.publication_lock_path().to_string())
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

    /// Reads the shard at `path` once in the section, runs `mutate`, and
    /// publishes the new shard when `mutate` returns `true`. A change to an
    /// enrolled row needs a grant in the permissions of the store.
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
            let contents = held.state_text(path)?;
            let bytes =
                crate::jsonl::mutate_jsonl_bytes(held, path, contents.as_deref(), |records| {
                    mutate(held, records)
                })?;
            let Some(bytes) = bytes else {
                return Ok(false);
            };
            held.publish_shard(path, bytes)?;
            Ok(true)
        })
    }

    /// Publishes the shard at `path` as a one-key write set.
    #[rule("rule_write_sets_publish_through_the_storage_port")]
    pub(crate) fn publish_shard(&self, path: &Utf8Path, bytes: Vec<u8>) -> anyhow::Result<()> {
        self.publish_set(WriteSet::new(Atomicity::PerKey).put(self.state_key(path)?, bytes))
    }

    /// Writes `records` as the shard at `path` in the stage of this store.
    /// The write carries no permission, so it cannot change an enrolled row.
    pub(crate) fn put_jsonl_records<T: Serialize>(
        &self,
        path: &Utf8Path,
        records: &[T],
    ) -> anyhow::Result<()> {
        protect_rows(self, path, records, &WritePermissions::none())?;
        let lines = records
            .iter()
            .map(crate::jsonl::to_stable_json)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut bytes = Vec::new();
        for line in lines {
            bytes.extend_from_slice(line.as_bytes());
            bytes.push(b'\n');
        }
        self.publish_shard(path, bytes)
    }

    /// Deletes every key below the state directory `directory` in the stage
    /// of this store.
    pub(crate) fn delete_state_keys_under(&self, directory: &Utf8Path) -> anyhow::Result<()> {
        let prefix = format!("{}/", self.state_key(directory)?.as_str());
        let Some(stage) = self.stage() else {
            anyhow::bail!("only a staged store deletes the keys below {directory}");
        };
        #[cfg(test)]
        if stage.reads_files() {
            if directory.exists() {
                std::fs::remove_dir_all(directory)?;
            }
            return Ok(());
        }
        let writes = stage.read(|view| {
            view.entries()
                .keys()
                .filter(|key| key.as_str().starts_with(&prefix))
                .fold(WriteSet::new(Atomicity::PerKey), |writes, key| {
                    writes.delete(key.clone())
                })
        });
        self.publish_set(writes)
    }

    /// Lands the changes of a stage that this store holds the section for:
    /// on the overlay of an outer stage, or through the adapter.
    pub(crate) fn land_stage(&self, writes: WriteSet) -> anyhow::Result<()> {
        self.publish_set(writes)
    }

    /// Publishes `writes`. A staged store lands them on the overlay of its
    /// stage. A store with a section view puts the revision of the view on
    /// a set that has no base, and applies the set to the view once it
    /// lands. Any other store publishes through its adapter.
    fn publish_set(&self, writes: WriteSet) -> anyhow::Result<()> {
        if let Some(stage) = self.stage() {
            writes
                .ensure_disjoint()
                .map_err(crate::write_error::publish_failure)?;
            #[cfg(test)]
            if stage.reads_files() {
                use crate::storage::StateStorage as _;
                return crate::storage::local::LocalStateStorage::new(self.layout.clone())
                    .publish(writes)
                    .map(drop)
                    .map_err(crate::write_error::publish_failure);
            }
            stage.apply(&writes);
            return Ok(());
        }
        self.publish_through(writes, |writes| self.storage().publish(writes))
    }

    /// Publishes `writes` with `publish`, in the section view of this store
    /// as [`Self::publish_set`] does.
    pub(crate) fn publish_through(
        &self,
        mut writes: WriteSet,
        publish: impl FnOnce(
            WriteSet,
        )
            -> Result<crate::storage::Revision, crate::storage::PublishFailure>,
    ) -> anyhow::Result<()> {
        let view = self.section_view();
        if writes.base().is_none() {
            if let Some(base) = view.and_then(crate::state_store::SectionView::revision) {
                writes = writes.on_base(base);
            }
        }
        let landed = publish(writes.clone());
        if let Some(view) = view {
            match &landed {
                Ok(_) => view.apply(&writes),
                // A started set can have landed in part, and a moved base
                // means that the state changed outside the section. Either
                // way, the next read of the section reads the state again.
                Err(
                    crate::storage::PublishFailure::Started(_)
                    | crate::storage::PublishFailure::BaseMoved { .. },
                ) => view.forget(),
                Err(crate::storage::PublishFailure::Refused(_)) => {}
            }
        }
        landed
            .map(drop)
            .map_err(crate::write_error::publish_failure)
    }
}

#[cfg(all(test, unix))]
mod containment_tests;
#[cfg(test)]
mod tests;
