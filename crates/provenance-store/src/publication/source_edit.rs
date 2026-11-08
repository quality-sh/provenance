use super::sync_directory;
use crate::storage::local::whole_set::{InstallState, WholeSetHook};
use crate::storage::local::{FileInstall, LocalStateStorage};
use crate::{
    layout::ProvenanceLayout,
    operations::files::{
        FileIdentity, HeldRepositoryFile, PreparedRepositoryFile, RepositoryFileBackup,
    },
};
use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::io::Write as _;

#[derive(Debug, thiserror::Error)]
pub enum SourceEditRecoveryFailure {
    #[error("source file changed outside the pending source-edit transaction")]
    ExternalChange,
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedSourceRecord {
    pub(super) target: Utf8PathBuf,
    pub(super) after_digest: [u8; 32],
    pub(super) prepared_leaf: String,
    pub(super) prepared_identity: FileIdentity,
}

/// Publishes a stage and one repository file edit under the live section
/// that `section` holds, through the local adapter of its layout: the edit
/// is a working-tree step, so it runs only on local disk.
pub fn with_staged_state_and_source_edit<R>(
    section: &crate::state_store::StateStore,
    held: HeldRepositoryFile,
    replacement: &[u8],
    prepare: impl FnOnce(&crate::state_store::StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    section.ensure_holds_live_section()?;
    let staged = section.begin_stage(crate::state_store::RecordStamp::ResolveFromRepository)?;
    let result = super::with_stage_body(&section.layout, || prepare(&staged))?;
    staged.ensure_stage_intact()?;
    let mut publication = SourceEditPublication {
        held: Some(held),
        replacement: replacement.to_vec(),
        prepared: None,
        backup: None,
        marker: None,
    };
    let local = LocalStateStorage::new(section.layout.clone());
    section.publish_through(staged.stage_changes()?, |writes| {
        local.publish_with_file(&writes, FileInstall(&mut publication))
    })?;
    Ok(result)
}

struct SourceEditPublication {
    held: Option<HeldRepositoryFile>,
    replacement: Vec<u8>,
    prepared: Option<PreparedRepositoryFile>,
    backup: Option<RepositoryFileBackup>,
    marker: Option<SourceEditMarker>,
}

impl WholeSetHook for SourceEditPublication {
    fn transactions_dir(&self, live: &ProvenanceLayout) -> Utf8PathBuf {
        live.source_edit_transactions_dir()
    }

    fn marker_path(&self, live: &ProvenanceLayout) -> Utf8PathBuf {
        live.source_edit_marker_path()
    }

    fn prepared(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        let replacement_path = transaction.join("replacement");
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
        let held = self.held.as_ref().expect("source-edit held file");
        let prepared = held
            .create_temp(&self.replacement)
            .context("prepare source replacement file")?;
        let backup = held.recovery_backup();
        let prepared_record = PreparedSourceRecord {
            target: held.relative().to_owned(),
            after_digest: Sha256::digest(&self.replacement).into(),
            prepared_leaf: prepared.recovery_leaf().to_owned(),
            prepared_identity: prepared.recovery_identity().clone(),
        };
        write_prepared_source_record(transaction, &prepared_record)
            .context("record prepared source file")?;
        self.prepared = Some(prepared);
        self.backup = Some(backup.clone());
        crate::test_probes::at("source_edit_temp_prepared")?;
        let marker = SourceEditMarker {
            transaction_dir: transaction.to_owned(),
            phase: SourceEditPhase::Prepared,
            target: held.relative().to_owned(),
            target_identity: held.identity().clone(),
            before_digest: held.digest(),
            after_digest: Sha256::digest(&self.replacement).into(),
            prepared_leaf: prepared_record.prepared_leaf,
            prepared_identity: prepared_record.prepared_identity,
            backup_leaf: backup.leaf().to_owned(),
            backup_identity: backup.identity().clone(),
            backup_digest: backup.digest(),
        };
        write_marker(live, &marker).context("write prepared source-edit marker")?;
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
        write_marker(live, marker)?;
        crate::test_probes::at("source_edit_backup_created")
    }

    fn state_installed(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()> {
        let marker = self.marker.as_mut().expect("source-edit marker");
        marker.phase = SourceEditPhase::StateInstalled;
        write_marker(live, marker)?;
        crate::test_probes::at("source_edit_state_installed")
    }

    fn install_file(&mut self) -> anyhow::Result<InstallState> {
        let held = self.held.take().expect("source-edit held file");
        let prepared = self.prepared.take().expect("source-edit prepared file");
        let backup = self.backup.take().expect("source-edit backup");
        held.compare_and_swap_with_backup(prepared, &backup)?;
        Ok(InstallState::FileInstalled)
    }

    fn published(
        &mut self,
        live: &ProvenanceLayout,
        _transaction: &Utf8Path,
    ) -> anyhow::Result<()> {
        let marker = self.marker.as_mut().expect("source-edit marker");
        marker.phase = SourceEditPhase::FileInstalled;
        crate::test_probes::at("source_edit_before_file_marker_write")?;
        write_marker(live, marker)
    }

    fn after_published(&mut self) -> anyhow::Result<()> {
        crate::test_probes::at("source_edit_file_installed")
    }

    fn rollback_finished(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()> {
        std::fs::remove_file(live.source_edit_marker_path())?;
        sync_directory(&live.cache_dir())
    }

    fn finish(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        finish(live, transaction)
    }
}

pub(super) fn write_marker(
    layout: &ProvenanceLayout,
    marker: &SourceEditMarker,
) -> anyhow::Result<()> {
    let mut temporary =
        tempfile::NamedTempFile::new_in(crate::long_path::extended(layout.cache_dir())?)?;
    temporary.write_all(&serde_json::to_vec(marker)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(crate::long_path::extended(
        layout.source_edit_marker_path(),
    )?)?;
    sync_directory(&layout.cache_dir())
}

pub(super) fn finish(layout: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
    std::fs::remove_dir_all(transaction)?;
    crate::test_probes::at("source_edit_transaction_removed")?;
    std::fs::remove_file(layout.source_edit_marker_path())?;
    sync_directory(&layout.cache_dir())
}

fn write_prepared_source_record(
    transaction: &Utf8Path,
    record: &PreparedSourceRecord,
) -> anyhow::Result<()> {
    let mut file = std::fs::File::create(transaction.join("prepared-source.json"))?;
    file.write_all(&serde_json::to_vec(record)?)?;
    file.sync_all()?;
    sync_directory(transaction)
}
