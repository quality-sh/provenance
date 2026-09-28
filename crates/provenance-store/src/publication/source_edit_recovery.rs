use super::{
    source_edit::{
        finish, write_marker, PreparedSourceRecord, SourceEditMarker, SourceEditPhase,
        SourceEditRecoveryFailure,
    },
    sync_directory,
};
use crate::{
    layout::ProvenanceLayout,
    operations::files::{
        validate_recovery_artifact_leaf, HeldRepositoryFile, RepositoryFileBackup,
        RepositoryFileRefusal, RepositoryFiles,
    },
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, SchemaVersion};
use sha2::{Digest as _, Sha256};

pub(super) fn recover_pending_source_edit(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let marker_path = layout.source_edit_marker_path();
    if !marker_path.exists() {
        return clean_unmarked_transactions(layout);
    }
    let mut marker: SourceEditMarker = serde_json::from_slice(&std::fs::read(&marker_path)?)?;
    ensure_supported_schema_version(
        "source-edit publication marker",
        SchemaVersion(marker.schema_version),
    )?;
    validate_marker_leaves(&marker)?;
    if marker.phase == SourceEditPhase::FileInstalled && !marker.transaction_dir.exists() {
        validate_missing_transaction(layout, &marker.transaction_dir)?;
        let current = read_current(layout, &marker)?;
        require_identity(&current, &marker, marker.after_digest)?;
        std::fs::remove_file(marker_path)?;
        return sync_directory(&layout.cache_dir());
    }
    let transaction = validate_transaction(layout, &marker.transaction_dir)?;
    validate_recovery_tree(&transaction)?;
    let replacement = read_regular_file(&transaction.join("replacement"))?;
    anyhow::ensure!(
        <[u8; 32]>::from(Sha256::digest(&replacement)) == marker.after_digest,
        "source-edit recovery replacement digest differs from its marker"
    );
    let files = RepositoryFiles::open(layout.root())?;
    let held = match files.read_bounded(&marker.target, usize::MAX) {
        Ok(held) => held,
        Err(RepositoryFileRefusal::Missing) => files
            .restore_recovery_backup(
                &marker.target,
                &marker.backup_leaf,
                &marker.backup_identity,
                marker.backup_digest,
            )
            .map_err(|_| SourceEditRecoveryFailure::ExternalChange)?,
        Err(_) => return Err(SourceEditRecoveryFailure::ExternalChange.into()),
    };
    let current = held.digest();
    if current != marker.before_digest && current != marker.after_digest {
        return Err(SourceEditRecoveryFailure::ExternalChange.into());
    }
    require_identity(&held, &marker, current)?;
    complete_state(layout, &transaction, &mut marker)?;
    if current == marker.before_digest {
        let prepared = match held.recover_temp(
            &marker.prepared_leaf,
            &marker.prepared_identity,
            marker.after_digest,
        )? {
            Some(prepared) => prepared,
            None => held.create_temp(&replacement)?,
        };
        let backup = RepositoryFileBackup::from_recovery(
            marker.backup_leaf.clone(),
            marker.backup_identity.clone(),
            marker.backup_digest,
        )?;
        held.compare_and_swap_with_backup(prepared, &backup)?;
        marker.phase = SourceEditPhase::FileInstalled;
        write_marker(layout, &marker)?;
        crate::test_probes::at("source_edit_file_installed")?;
    } else if let Some(prepared) = held.recover_temp(
        &marker.prepared_leaf,
        &marker.prepared_identity,
        marker.after_digest,
    )? {
        drop(prepared);
    }
    finish(layout, &transaction)
}

fn require_identity(
    current: &HeldRepositoryFile,
    marker: &SourceEditMarker,
    digest: [u8; 32],
) -> anyhow::Result<()> {
    let expected = if digest == marker.before_digest {
        &marker.target_identity
    } else {
        &marker.prepared_identity
    };
    if current.identity() != expected {
        return Err(SourceEditRecoveryFailure::ExternalChange.into());
    }
    Ok(())
}

fn read_current(
    layout: &ProvenanceLayout,
    marker: &SourceEditMarker,
) -> anyhow::Result<HeldRepositoryFile> {
    RepositoryFiles::open(layout.root())?
        .read_bounded(&marker.target, usize::MAX)
        .map_err(|_| SourceEditRecoveryFailure::ExternalChange.into())
}

fn validate_marker_leaves(marker: &SourceEditMarker) -> anyhow::Result<()> {
    validate_recovery_artifact_leaf(&marker.target, &marker.prepared_leaf, ".tmp")?;
    validate_recovery_artifact_leaf(&marker.target, &marker.backup_leaf, ".backup")?;
    Ok(())
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
            crate::test_probes::at("state_after_backup_rename")?;
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
            crate::test_probes::at("state_after_install_rename")?;
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

fn clean_unmarked_transactions(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(layout.source_edit_transactions_dir())? {
        let path = Utf8PathBuf::from_path_buf(entry?.path()).map_err(|path| {
            anyhow::anyhow!("source-edit transaction is not UTF-8: {}", path.display())
        })?;
        let transaction = validate_transaction(layout, &path)?;
        let record: PreparedSourceRecord = serde_json::from_slice(&read_regular_file(
            &transaction.join("prepared-source.json"),
        )?)?;
        validate_recovery_artifact_leaf(&record.target, &record.prepared_leaf, ".tmp")?;
        RepositoryFiles::open(layout.root())?.remove_recovery_temp(
            &record.target,
            &record.prepared_leaf,
            &record.prepared_identity,
            record.after_digest,
        )?;
        std::fs::remove_dir_all(transaction)?;
    }
    Ok(())
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
    require_real_directory(transaction)?;
    Utf8PathBuf::from_path_buf(std::fs::canonicalize(transaction)?)
        .map_err(|path| anyhow::anyhow!("source-edit transaction is not UTF-8: {}", path.display()))
}

fn validate_recovery_tree(transaction: &Utf8Path) -> anyhow::Result<()> {
    let staged_repo = transaction.join("staged-repo");
    require_real_directory(&staged_repo)?;
    let staged = ProvenanceLayout::new(staged_repo);
    require_real_directory(&staged.provenance_dir())?;
    require_real_directory_if_present(&staged.state_dir())?;
    require_real_directory_if_present(&transaction.join("backup-state"))?;
    Ok(())
}

fn require_real_directory(path: &Utf8Path) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "source-edit recovery path is not a real directory: {path}"
    );
    Ok(())
}

fn require_real_directory_if_present(path: &Utf8Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "source-edit recovery path is not a real directory: {path}"
            );
            Ok(())
        }
    }
}

fn read_regular_file(path: &Utf8Path) -> anyhow::Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "source-edit recovery path is not a real file: {path}"
    );
    Ok(std::fs::read(path)?)
}

fn validate_missing_transaction(
    layout: &ProvenanceLayout,
    transaction: &Utf8Path,
) -> anyhow::Result<()> {
    let parent = std::fs::canonicalize(layout.source_edit_transactions_dir())?;
    let candidate_parent = transaction
        .parent()
        .ok_or_else(|| anyhow::anyhow!("source-edit transaction has no parent"))?;
    anyhow::ensure!(
        std::fs::canonicalize(candidate_parent)? == parent,
        "source-edit transaction is outside its transaction directory"
    );
    match std::fs::symlink_metadata(transaction) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("source-edit transaction still exists"),
    }
}
