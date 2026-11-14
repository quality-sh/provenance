use super::durability::sync_directory;
use super::transactions::{
    contained_transaction, end_marked_write, read_marker, remove_leftovers, remove_unpublished,
    write_marker, TransactionKind,
};
use crate::layout::ProvenanceLayout;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

#[derive(Clone, Copy, Debug, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationPhase {
    Prepared,
    BackupCreated,
    Published,
}

#[derive(serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicationMarker {
    transaction_dir: Utf8PathBuf,
    phase: PublicationPhase,
}

pub fn write_publication_marker(
    layout: &ProvenanceLayout,
    transaction_dir: &Utf8Path,
    phase: PublicationPhase,
) -> anyhow::Result<()> {
    let transaction_dir = validated_transaction_dir(layout, transaction_dir)?;
    let marker = PublicationMarker {
        transaction_dir,
        phase,
    };
    write_marker(
        layout,
        TransactionKind::Import,
        &serde_json::to_vec(&marker)?,
    )
}

/// Completes or rolls back the publication that the marker names, and then
/// removes the data of each import write that stopped before its marker.
///
/// The caller holds the section lock: with no lock, the removal can remove
/// the transaction of a live write.
pub(super) fn recover_pending_publication(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    if let Some(marker) = read_marker(layout, TransactionKind::Import)? {
        recover_marked_publication(layout, &marker)?;
    }
    remove_leftovers(layout, TransactionKind::Import, remove_unpublished)
}

/// Puts the backup state in place when the live state is gone, and then ends
/// the write that the marker names.
fn recover_marked_publication(
    layout: &ProvenanceLayout,
    marker: &PublicationMarker,
) -> anyhow::Result<()> {
    let published = matches!(marker.phase, PublicationPhase::Published);
    if !published || marker.transaction_dir.exists() {
        let transaction = validated_transaction_dir(layout, &marker.transaction_dir)?;
        let backup = transaction.join("backup-state");
        if !layout.state_dir().exists() {
            crate::write_error::ensure!(
                Fault,
                backup.exists(),
                "publication recovery found neither live state nor backup state"
            );
            std::fs::rename(&backup, layout.state_dir())?;
            sync_directory(&layout.provenance_dir())?;
        }
    }
    end_marked_write(layout, TransactionKind::Import, &marker.transaction_dir)
}

/// The resolved path of the import transaction at `transaction_dir`.
pub fn validated_transaction_dir(
    layout: &ProvenanceLayout,
    transaction_dir: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    contained_transaction(layout, TransactionKind::Import, transaction_dir)
}

pub fn create_real_directory(path: &Utf8Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Err(error) = std::fs::create_dir(path) {
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error).with_context(|| {
                        format!("failed to create publication lock directory {path}")
                    });
                }
            }
        }
        Err(error) => return Err(error.into()),
    }
    let metadata = std::fs::symlink_metadata(path)?;
    crate::write_error::ensure!(
        Fault,
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "publication lock path contains a symlink component: {path}"
    );
    Ok(())
}
