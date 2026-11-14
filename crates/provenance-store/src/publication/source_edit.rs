use super::source_edit_recovery::remove_unpublished_source_edit;
use super::sync_directory;
use crate::storage::local::durability::write_synced_file;
use crate::storage::local::whole_set::{InstallState, WholeSetHook};
use crate::storage::local::{
    removal_failure, write_marker, FileInstall, RemovalStep, TransactionKind,
};
use crate::{
    layout::ProvenanceLayout,
    operations::files::{
        FileIdentity, HeldRepositoryFile, PreparedRepositoryFile, RepositoryFileBackup,
        RepositoryFileRefusal,
    },
};
use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum SourceEditRecoveryFailure {
    #[error("source file {file} changed outside the pending source-edit transaction")]
    ExternalChange { file: Utf8PathBuf },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceEditPhase {
    Prepared,
    BackupCreated,
    StateInstalled,
    FileInstalled,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceEditMarker {
    pub(super) transaction_dir: Utf8PathBuf,
    pub(super) phase: SourceEditPhase,
    pub(super) target: Utf8PathBuf,
    pub(super) target_identity: FileIdentity,
    pub(super) before_digest: [u8; 32],
    pub(super) after_digest: [u8; 32],
    pub(super) prepared_leaf: String,
    pub(super) prepared_identity: FileIdentity,
    pub(super) backup_leaf: String,
    pub(super) backup_identity: FileIdentity,
    pub(super) backup_digest: [u8; 32],
}

/// The record of the prepared file of a source edit, in its transaction.
/// The store writes it before it makes the file, so that a stop at any later
/// point leaves a record that names the file.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedSourceRecord {
    pub(super) target: Utf8PathBuf,
    pub(super) prepared_leaf: String,
}

/// Publishes a stage and one repository file edit under the live section
/// that `section` holds, through the local adapter of its layout.
///
/// The edit is a working-tree step, so it runs only on local disk. A
/// repository on another adapter is refused before the stage starts.
pub fn with_staged_state_and_source_edit<R>(
    section: &crate::state_store::StateStore,
    held: HeldRepositoryFile,
    replacement: &[u8],
    prepare: impl FnOnce(&crate::state_store::StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    section.ensure_holds_live_section()?;
    let local = section.local_storage()?;
    let staged = section.begin_stage(crate::state_store::RecordStamp::ResolveFromRepository)?;
    let result = super::with_stage_body(&section.layout, || prepare(&staged))?;
    staged.ensure_stage_intact()?;
    let mut publication = SourceEditPublication {
        held: Some(held),
        replacement: replacement.to_vec(),
        prepared: None,
        marker: None,
    };
    section.publish_through(staged.stage_changes()?, |writes| {
        local.publish_with_file(&writes, FileInstall(&mut publication))
    })?;
    Ok(result)
}

struct SourceEditPublication {
    held: Option<HeldRepositoryFile>,
    replacement: Vec<u8>,
    prepared: Option<PreparedRepositoryFile>,
    marker: Option<SourceEditMarker>,
}

impl WholeSetHook for SourceEditPublication {
    fn kind(&self) -> TransactionKind {
        TransactionKind::SourceEdit
    }

    /// Writes the record of the prepared file, then the replacement, each
    /// synced, and then makes the prepared file. Thus a prepared file exists
    /// only when its record and the complete replacement are durable.
    fn prepared(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        let held = self.held.as_ref().expect("source-edit held file");
        let prepared_record = PreparedSourceRecord {
            target: held.relative().to_owned(),
            prepared_leaf: held.new_temp_leaf(),
        };
        write_prepared_source_record(transaction, &prepared_record)
            .context("record prepared source file")?;
        let replacement_path = transaction.join(REPLACEMENT);
        std::fs::write(&replacement_path, &self.replacement)
            .context("write source-edit replacement record")?;
        std::fs::OpenOptions::new()
            .read(true)
            .write(cfg!(windows))
            .open(&replacement_path)
            .context("open source-edit replacement record")?
            .sync_all()
            .context("flush source-edit replacement record")?;
        sync_directory(transaction).context("flush source-edit transaction")?;
        let prepared = held
            .create_temp_at(&prepared_record.prepared_leaf, &self.replacement)
            .context("prepare source replacement file")?;
        let prepared_identity = prepared.recovery_identity().clone();
        let backup = held.recovery_backup();
        self.prepared = Some(prepared);
        crate::test_probes::at("source_edit_temp_prepared")?;
        let marker = SourceEditMarker {
            transaction_dir: transaction.to_owned(),
            phase: SourceEditPhase::Prepared,
            target: held.relative().to_owned(),
            target_identity: held.identity().clone(),
            before_digest: held.digest(),
            after_digest: Sha256::digest(&self.replacement).into(),
            prepared_leaf: prepared_record.prepared_leaf,
            prepared_identity,
            backup_leaf: backup.leaf().to_owned(),
            backup_identity: backup.identity().clone(),
            backup_digest: backup.digest(),
        };
        write_source_edit_marker(live, &marker).context("write prepared source-edit marker")?;
        self.marker = Some(marker);
        crate::test_probes::at("source_edit_prepared")
    }

    fn backup_created(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &Utf8Path,
    ) -> anyhow::Result<()> {
        sync_directory(transaction)?;
        sync_directory(&live.provenance_dir())?;
        let marker = self.marker.as_mut().expect("source-edit marker");
        marker.phase = SourceEditPhase::BackupCreated;
        write_source_edit_marker(live, marker)?;
        crate::test_probes::at("source_edit_backup_created")
    }

    fn state_installed(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()> {
        let marker = self.marker.as_mut().expect("source-edit marker");
        marker.phase = SourceEditPhase::StateInstalled;
        write_source_edit_marker(live, marker)?;
        crate::test_probes::at("source_edit_state_installed")
    }

    /// A failed rename back of the target leaves the initial file in the
    /// backup, so the write keeps its marker, and the next section entry
    /// puts the backup back and completes the install.
    fn install_file(
        &mut self,
        live: &ProvenanceLayout,
    ) -> Result<InstallState, (InstallState, anyhow::Error)> {
        let held = self.held.as_ref().expect("source-edit held file");
        let prepared = self.prepared.take().expect("source-edit prepared file");
        let marker = self.marker.as_ref().expect("source-edit marker");
        let backup = backup(marker).map_err(|error| (InstallState::StateOnly, error))?;
        match held.install(prepared, &backup) {
            Ok(()) => Ok(InstallState::FileMoved),
            Err(RepositoryFileRefusal::Restore(error)) => {
                let path = artifact(live, marker, &marker.backup_leaf);
                let error = removal_failure(RemovalStep::Restore, &path, error);
                Err((InstallState::FileMoved, error))
            }
            Err(refusal) => Err((InstallState::StateOnly, refusal.into())),
        }
    }

    fn published(
        &mut self,
        live: &ProvenanceLayout,
        _transaction: &Utf8Path,
    ) -> anyhow::Result<()> {
        let held = self.held.as_ref().expect("source-edit held file");
        let marker = self.marker.as_mut().expect("source-edit marker");
        complete_install(live, held, marker, &self.replacement)
    }

    fn after_published(&mut self) -> anyhow::Result<()> {
        crate::test_probes::at("source_edit_file_installed")
    }

    /// Closes the prepared file, with no removal, and then removes it through
    /// the record of the transaction, as the next section entry does.
    fn remove_unpublished(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &Utf8Path,
    ) -> anyhow::Result<()> {
        self.prepared = None;
        remove_unpublished_source_edit(live, transaction)
    }
}

pub(super) fn write_source_edit_marker(
    layout: &ProvenanceLayout,
    marker: &SourceEditMarker,
) -> anyhow::Result<()> {
    write_marker(
        layout,
        TransactionKind::SourceEdit,
        &serde_json::to_vec(marker)?,
    )
}

/// Completes the install after the rename of the prepared file onto the
/// target: removes the store's own prepared file, syncs the directory of the
/// target, removes the backup that holds the file that the write read, syncs
/// that directory again, and then writes the `FileInstalled` marker. Thus
/// the install is durable before the backup goes. The live write and the
/// recovery use it, so a failure keeps the earlier marker, and the next
/// section entry runs it again.
pub(super) fn complete_install(
    layout: &ProvenanceLayout,
    held: &HeldRepositoryFile,
    marker: &mut SourceEditMarker,
    replacement: &[u8],
) -> anyhow::Result<()> {
    let prepared = artifact(layout, marker, &marker.prepared_leaf);
    let backup_path = artifact(layout, marker, &marker.backup_leaf);
    let directory = backup_path.parent().unwrap_or_else(|| layout.root());
    let unsynced = |error| removal_failure(RemovalStep::Sync, directory, error);
    held.remove_own_prepared(&marker.prepared_leaf, replacement)
        .map_err(|error| removal_failure(RemovalStep::PreparedFile, &prepared, error))?;
    held.sync_directory().map_err(unsynced)?;
    held.remove_own_backup(&backup(marker)?)
        .map_err(|error| removal_failure(RemovalStep::Backup, &backup_path, error))?;
    held.sync_directory().map_err(unsynced)?;
    marker.phase = SourceEditPhase::FileInstalled;
    crate::test_probes::at("source_edit_before_file_marker_write")?;
    write_source_edit_marker(layout, marker)
}

/// The path of the working-tree file `leaf` next to the target of `marker`.
fn artifact(layout: &ProvenanceLayout, marker: &SourceEditMarker, leaf: &str) -> Utf8PathBuf {
    layout.root().join(&marker.target).with_file_name(leaf)
}

/// The backup of the target that `marker` records.
pub(super) fn backup(marker: &SourceEditMarker) -> anyhow::Result<RepositoryFileBackup> {
    Ok(RepositoryFileBackup::from_recovery(
        marker.backup_leaf.clone(),
        marker.backup_identity.clone(),
        marker.backup_digest,
    )?)
}

/// The file of a source-edit transaction that holds the replacement bytes.
pub(super) const REPLACEMENT: &str = "replacement";

/// The file of a source-edit transaction that holds its record.
pub(super) const PREPARED_SOURCE_RECORD: &str = "prepared-source.json";

/// Writes `record` in `transaction` with a replace and a sync, so that the
/// record is complete or does not exist: recovery reads a record that exists.
fn write_prepared_source_record(
    transaction: &Utf8Path,
    record: &PreparedSourceRecord,
) -> anyhow::Result<()> {
    write_synced_file(
        &transaction.join(PREPARED_SOURCE_RECORD),
        &serde_json::to_vec(record)?,
    )
}
