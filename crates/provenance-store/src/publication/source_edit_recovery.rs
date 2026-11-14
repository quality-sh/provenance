use super::{
    source_edit::{
        end_published, write_source_edit_marker, PreparedSourceRecord, SourceEditMarker,
        SourceEditPhase, SourceEditRecoveryFailure, PREPARED_SOURCE_RECORD, REPLACEMENT,
    },
    sync_directory,
};
use crate::{
    layout::ProvenanceLayout,
    operations::files::{
        validate_recovery_artifact_leaf, HeldRepositoryFile, RepositoryFileBackup,
        RepositoryFileRefusal, RepositoryFiles,
    },
    storage::local::{contained_transaction, remove_leftovers, TransactionKind},
};
use anyhow::Context as _;
use camino::Utf8Path;
use sha2::{Digest as _, Sha256};

/// Completes the source edit that the marker names, and then removes the
/// data of each source edit that stopped before its marker, with the
/// working-tree file that the record of its transaction names.
///
/// The caller holds the section lock: with no lock, the removal can remove
/// the transaction of a live write.
pub fn recover_pending_source_edit(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let marker_path = layout.source_edit_marker_path();
    if marker_path.exists() {
        let mut marker: SourceEditMarker = serde_json::from_slice(&std::fs::read(&marker_path)?)?;
        validate_marker_leaves(&marker)?;
        recover_marked_source_edit(layout, &mut marker)?;
    }
    remove_leftovers(layout, TransactionKind::SourceEdit, |transaction| {
        remove_prepared_file(layout, transaction)
    })
}

/// A marker before `FileInstalled` needs the full transaction. The recovery
/// writes the `FileInstalled` marker before it removes the transaction, so a
/// removal that stops or fails leaves a marker that needs no transaction file.
fn recover_marked_source_edit(
    layout: &ProvenanceLayout,
    marker: &mut SourceEditMarker,
) -> anyhow::Result<()> {
    if marker.phase != SourceEditPhase::FileInstalled {
        let transaction =
            contained_transaction(layout, TransactionKind::SourceEdit, &marker.transaction_dir)?;
        validate_recovery_tree(&transaction)?;
        let replacement = read_replacement(&transaction, marker.after_digest)?;
        let held = recover_target(layout, marker)?;
        let current = require_known_target(&held, marker)?;
        complete_state(layout, &transaction, marker)?;
        complete_file(layout, &held, marker, current, &replacement)?;
    }
    end_published(layout, &marker.transaction_dir)
}

fn read_replacement(transaction: &Utf8Path, expected: [u8; 32]) -> anyhow::Result<Vec<u8>> {
    let replacement = read_regular_file(&transaction.join(REPLACEMENT))?;
    crate::write_error::ensure!(
        Fault,
        <[u8; 32]>::from(Sha256::digest(&replacement)) == expected,
        "source-edit recovery replacement digest differs from its marker"
    );
    Ok(replacement)
}

fn recover_target(
    layout: &ProvenanceLayout,
    marker: &SourceEditMarker,
) -> anyhow::Result<HeldRepositoryFile> {
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
    Ok(held)
}

fn require_known_target(
    held: &HeldRepositoryFile,
    marker: &SourceEditMarker,
) -> anyhow::Result<[u8; 32]> {
    let current = held.digest();
    if current != marker.before_digest && current != marker.after_digest {
        return Err(SourceEditRecoveryFailure::ExternalChange.into());
    }
    require_identity(held, marker, current)?;
    Ok(current)
}

/// Completes the install of the file, and then writes the `FileInstalled`
/// marker: the install and the removal of the store's own prepared file and
/// backup are durable before that marker.
fn complete_file(
    layout: &ProvenanceLayout,
    held: &HeldRepositoryFile,
    marker: &mut SourceEditMarker,
    current: [u8; 32],
    replacement: &[u8],
) -> anyhow::Result<()> {
    if current == marker.before_digest {
        install_replacement(layout, held, marker, replacement)?;
    } else {
        held.remove_own_prepared(&marker.prepared_leaf, replacement)
            .with_context(|| removal(layout, &marker.target, &marker.prepared_leaf))?;
        held.finish_recovered_install(&backup(marker)?)?;
    }
    marker.phase = SourceEditPhase::FileInstalled;
    write_source_edit_marker(layout, marker)?;
    crate::test_probes::at("source_edit_file_installed")
}

/// Installs the replacement with the prepared file that the marker names.
/// When that file is not the one that the marker records, the recovery
/// removes it if it is the store's own data, makes it again at the same
/// name, and records its identity before the swap.
fn install_replacement(
    layout: &ProvenanceLayout,
    held: &HeldRepositoryFile,
    marker: &mut SourceEditMarker,
    replacement: &[u8],
) -> anyhow::Result<()> {
    let leaf = marker.prepared_leaf.clone();
    let prepared = match held.recover_temp(&leaf, &marker.prepared_identity, marker.after_digest) {
        Ok(Some(prepared)) => prepared,
        Ok(None) | Err(RepositoryFileRefusal::Changed | RepositoryFileRefusal::InvalidUtf8) => {
            held.remove_own_prepared(&leaf, replacement)
                .with_context(|| removal(layout, &marker.target, &leaf))?;
            let prepared = held.create_temp_at(&leaf, replacement)?;
            marker.prepared_identity = prepared.recovery_identity().clone();
            write_source_edit_marker(layout, marker)?;
            prepared
        }
        Err(error) => return Err(error.into()),
    };
    held.compare_and_swap_with_backup(prepared, &backup(marker)?)?;
    Ok(())
}

fn backup(marker: &SourceEditMarker) -> anyhow::Result<RepositoryFileBackup> {
    Ok(RepositoryFileBackup::from_recovery(
        marker.backup_leaf.clone(),
        marker.backup_identity.clone(),
        marker.backup_digest,
    )?)
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
    if marker.phase == SourceEditPhase::Prepared {
        complete_backup_phase(layout, transaction, marker)?;
    }
    if marker.phase == SourceEditPhase::BackupCreated {
        complete_install_phase(layout, transaction, marker)?;
    }
    Ok(())
}

fn complete_backup_phase(
    layout: &ProvenanceLayout,
    transaction: &Utf8Path,
    marker: &mut SourceEditMarker,
) -> anyhow::Result<()> {
    let backup = transaction.join("backup-state");
    if layout.state_dir().exists() && !backup.exists() {
        std::fs::rename(layout.state_dir(), &backup)?;
        crate::test_probes::at("state_after_backup_rename")?;
    }
    marker.phase = SourceEditPhase::BackupCreated;
    write_source_edit_marker(layout, marker)?;
    crate::test_probes::at("source_edit_backup_created")
}

fn complete_install_phase(
    layout: &ProvenanceLayout,
    transaction: &Utf8Path,
    marker: &mut SourceEditMarker,
) -> anyhow::Result<()> {
    let staged = ProvenanceLayout::new(transaction.join("staged-repo"));
    if staged.state_dir().exists() {
        install_staged_state(layout, &staged)?;
    } else {
        crate::write_error::ensure!(
            Fault,
            layout.state_dir().exists(),
            "source-edit recovery found no state"
        );
    }
    marker.phase = SourceEditPhase::StateInstalled;
    write_source_edit_marker(layout, marker)?;
    crate::test_probes::at("source_edit_state_installed")
}

fn install_staged_state(
    layout: &ProvenanceLayout,
    staged: &ProvenanceLayout,
) -> anyhow::Result<()> {
    crate::write_error::ensure!(
        Fault,
        !layout.state_dir().exists(),
        "source-edit recovery found both staged and live state"
    );
    std::fs::rename(staged.state_dir(), layout.state_dir())?;
    crate::test_probes::at("state_after_install_rename")?;
    sync_directory(&layout.provenance_dir())
}

/// Removes the prepared working-tree file that the record of `transaction`
/// names, when the file is the store's own data. A write records the file
/// before it makes it, so a transaction with no record has no prepared file.
/// A write makes the file after the replacement is complete, so with no
/// replacement, a file with that name is not the store's.
fn remove_prepared_file(layout: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
    let Some(record) = read_if_present(&transaction.join(PREPARED_SOURCE_RECORD))? else {
        return Ok(());
    };
    let record: PreparedSourceRecord = serde_json::from_slice(&record)?;
    validate_recovery_artifact_leaf(&record.target, &record.prepared_leaf, ".tmp")?;
    let replacement = read_if_present(&transaction.join(REPLACEMENT))?;
    RepositoryFiles::open(layout.root())?
        .remove_own_prepared(&record.target, &record.prepared_leaf, replacement.as_deref())
        .with_context(|| removal(layout, &record.target, &record.prepared_leaf))?;
    Ok(())
}

/// The step of the removal of the prepared file `leaf` next to `target`.
fn removal(layout: &ProvenanceLayout, target: &Utf8Path, leaf: &str) -> String {
    let file = layout.root().join(target).with_file_name(leaf);
    format!("remove prepared file {file}")
}

/// The bytes of the regular file at `path`, or `None` when no entry is there.
fn read_if_present(path: &Utf8Path) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(_) => read_regular_file(path).map(Some),
    }
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
    crate::write_error::ensure!(
        Fault,
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
            crate::write_error::ensure!(
                Fault,
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "source-edit recovery path is not a real directory: {path}"
            );
            Ok(())
        }
    }
}

fn read_regular_file(path: &Utf8Path) -> anyhow::Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    crate::write_error::ensure!(
        Fault,
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "source-edit recovery path is not a real file: {path}"
    );
    Ok(std::fs::read(path)?)
}
