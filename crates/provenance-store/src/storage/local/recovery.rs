use super::in_place::roll_forward;
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
    /// The change list of the transaction is complete: recovery installs it.
    Installing,
    /// Each change is installed: recovery removes the transaction data.
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

/// Completes the publication that the marker names, and then
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

/// Completes the install of a write that stopped after its marker, records
/// it as published, and then ends the write that the marker names.
fn recover_marked_publication(
    layout: &ProvenanceLayout,
    marker: &PublicationMarker,
) -> anyhow::Result<()> {
    if matches!(marker.phase, PublicationPhase::Installing) {
        let transaction = validated_transaction_dir(layout, &marker.transaction_dir)?;
        roll_forward(layout, &transaction)?;
        write_publication_marker(layout, &transaction, PublicationPhase::Published)?;
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
