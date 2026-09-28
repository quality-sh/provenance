use super::{staged::copy_directory, sync_directory, sync_tree, with_repository_publication};
use crate::{
    layout::ProvenanceLayout,
    operations::files::{FileIdentity, HeldRepositoryFile, RepositoryFiles},
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, SchemaVersion, SUPPORTED_SCHEMA_VERSION};
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
enum SourceEditPhase {
    Prepared,
    BackupCreated,
    StateInstalled,
    FileInstalled,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceEditMarker {
    schema_version: u32,
    transaction_dir: Utf8PathBuf,
    phase: SourceEditPhase,
    target: Utf8PathBuf,
    target_identity: FileIdentity,
    before_digest: [u8; 32],
    after_digest: [u8; 32],
    prepared_leaf: String,
    prepared_identity: FileIdentity,
}

pub fn with_staged_state_and_source_edit<R>(
    live: &ProvenanceLayout,
    held: HeldRepositoryFile,
    replacement: &[u8],
    prepare: impl FnOnce(&ProvenanceLayout) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    with_repository_publication(live, || publish(live, held, replacement, prepare)).map_err(
        |error| {
            if live.source_edit_marker_path().exists() {
                crate::write_error::publication_started(error)
            } else {
                error
            }
        },
    )
}

fn publish<R>(
    live: &ProvenanceLayout,
    held: HeldRepositoryFile,
    replacement: &[u8],
    prepare: impl FnOnce(&ProvenanceLayout) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let transaction = create_transaction(live)?;
    let staged_root = transaction.join("staged-repo");
    let staged = ProvenanceLayout::new(staged_root.clone());
    let result = (|| {
        copy_directory(&live.state_dir(), &staged.state_dir())?;
        let result = prepare(&staged)?;
        sync_tree(&staged.state_dir())?;
        sync_directory(&staged.provenance_dir())?;
        sync_directory(&staged_root)?;
        let replacement_path = transaction.join("replacement");
        std::fs::write(&replacement_path, replacement)?;
        std::fs::File::open(&replacement_path)?.sync_all()?;
        let prepared = held.create_temp(replacement)?;
        let mut marker = SourceEditMarker {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            transaction_dir: transaction.clone(),
            phase: SourceEditPhase::Prepared,
            target: held.relative().to_owned(),
            target_identity: held.identity().clone(),
            before_digest: held.digest(),
            after_digest: Sha256::digest(replacement).into(),
            prepared_leaf: prepared.recovery_leaf().to_owned(),
            prepared_identity: prepared.recovery_identity().clone(),
        };
        write_marker(live, &marker)?;
        crate::test_probes::at("source_edit_prepared")?;
        let backup = transaction.join("backup-state");
        std::fs::rename(live.state_dir(), &backup)?;
        marker.phase = SourceEditPhase::BackupCreated;
        write_marker(live, &marker)?;
        crate::test_probes::at("source_edit_backup_created")?;
        std::fs::rename(staged.state_dir(), live.state_dir())?;
        sync_directory(&live.provenance_dir())?;
        marker.phase = SourceEditPhase::StateInstalled;
        write_marker(live, &marker)?;
        crate::test_probes::at("source_edit_state_installed")?;
        held.compare_and_swap(prepared)?;
        marker.phase = SourceEditPhase::FileInstalled;
        write_marker(live, &marker)?;
        crate::test_probes::at("source_edit_file_installed")?;
        finish(live, &transaction)?;
        Ok(result)
    })();
    if !live.source_edit_marker_path().exists() && transaction.exists() {
        let _ = std::fs::remove_dir_all(&transaction);
    }
    result
}

pub(super) fn recover_pending_source_edit(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let marker_path = layout.source_edit_marker_path();
    if !marker_path.exists() {
        return Ok(());
    }
    let mut marker: SourceEditMarker = serde_json::from_slice(&std::fs::read(&marker_path)?)?;
    ensure_supported_schema_version(
        "source-edit publication marker",
        SchemaVersion(marker.schema_version),
    )?;
    let transaction = validate_transaction(layout, &marker.transaction_dir)?;
    let replacement = std::fs::read(transaction.join("replacement"))?;
    anyhow::ensure!(
        <[u8; 32]>::from(Sha256::digest(&replacement)) == marker.after_digest,
        "source-edit recovery replacement digest differs from its marker"
    );
    let files = RepositoryFiles::open(layout.root())?;
    let held = files
        .read_bounded(&marker.target, usize::MAX)
        .map_err(|_| SourceEditRecoveryFailure::ExternalChange)?;
    let current = held.digest();
    if current != marker.before_digest && current != marker.after_digest {
        return Err(SourceEditRecoveryFailure::ExternalChange.into());
    }
    complete_state(layout, &transaction, &mut marker)?;
    if current == marker.before_digest {
        let prepared = held.create_temp(&replacement)?;
        held.compare_and_swap(prepared)?;
        marker.phase = SourceEditPhase::FileInstalled;
        write_marker(layout, &marker)?;
        crate::test_probes::at("source_edit_file_installed")?;
    }
    finish(layout, &transaction)
}

fn complete_state(
    layout: &ProvenanceLayout,
    transaction: &Utf8Path,
    marker: &mut SourceEditMarker,
) -> anyhow::Result<()> {
    let staged = ProvenanceLayout::new(transaction.join("staged-repo"));
    let backup = transaction.join("backup-state");
    if marker.phase == SourceEditPhase::Prepared {
        if layout.state_dir().exists() && !backup.exists() {
            std::fs::rename(layout.state_dir(), &backup)?;
        }
        marker.phase = SourceEditPhase::BackupCreated;
        write_marker(layout, marker)?;
        crate::test_probes::at("source_edit_backup_created")?;
    }
    if marker.phase == SourceEditPhase::BackupCreated {
        if staged.state_dir().exists() {
            anyhow::ensure!(
                !layout.state_dir().exists(),
                "source-edit recovery found both staged and live state"
            );
            std::fs::rename(staged.state_dir(), layout.state_dir())?;
            sync_directory(&layout.provenance_dir())?;
        } else {
            anyhow::ensure!(
                layout.state_dir().exists(),
                "source-edit recovery found no state"
            );
        }
        marker.phase = SourceEditPhase::StateInstalled;
        write_marker(layout, marker)?;
        crate::test_probes::at("source_edit_state_installed")?;
    }
    Ok(())
}

fn create_transaction(layout: &ProvenanceLayout) -> anyhow::Result<Utf8PathBuf> {
    let name = format!("{}-{}", std::process::id(), uuid::Uuid::new_v4().simple());
    let path = layout.source_edit_transactions_dir().join(name);
    std::fs::create_dir(&path)?;
    Ok(path)
}

fn validate_transaction(
    layout: &ProvenanceLayout,
    transaction: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    let parent = std::fs::canonicalize(layout.source_edit_transactions_dir())?;
    let candidate_parent = transaction
        .parent()
        .ok_or_else(|| anyhow::anyhow!("source-edit transaction has no parent"))?;
    anyhow::ensure!(
        std::fs::canonicalize(candidate_parent)? == parent,
        "source-edit transaction is outside its transaction directory"
    );
    let metadata = std::fs::symlink_metadata(transaction)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "source-edit transaction is not a real directory"
    );
    Utf8PathBuf::from_path_buf(std::fs::canonicalize(transaction)?)
        .map_err(|path| anyhow::anyhow!("source-edit transaction path is not UTF-8: {}", path.display()))
}

fn write_marker(layout: &ProvenanceLayout, marker: &SourceEditMarker) -> anyhow::Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(layout.cache_dir())?;
    temporary.write_all(&serde_json::to_vec(marker)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(layout.source_edit_marker_path())?;
    sync_directory(&layout.cache_dir())
}

fn finish(layout: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
    std::fs::remove_dir_all(transaction)?;
    std::fs::remove_file(layout.source_edit_marker_path())?;
    sync_directory(&layout.cache_dir())
}
