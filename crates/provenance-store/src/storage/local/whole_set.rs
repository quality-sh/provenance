//! A whole set of many keys on local disk, installed file by file in the
//! live state directory (see `in_place`), with at most one working-tree file.
//!
//! The transaction lives under `.provenance/cache`. A marker names its phase,
//! so the next section entry completes a write that stopped after its
//! marker. A transaction that no marker names belongs to no live write: the
//! write removes it when it fails, and the next section entry removes it
//! after a stop.

use super::durability::sync_directory;
use super::in_place;
use super::recovery::{write_publication_marker, PublicationPhase};
use super::transactions::{clear_marker, end_marked_write, remove_unpublished, TransactionKind};
use crate::layout::ProvenanceLayout;
use crate::storage::WriteSet;
use camino::Utf8Path;
use provenance_macros::rule;

/// The steps that a publication names in one whole-set transaction.
///
/// The steps say the kind of the transaction and its marker, what each phase
/// records, and which working-tree file lands with the state, if any.
pub trait WholeSetHook {
    fn kind(&self) -> TransactionKind;
    fn prepared(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()>;
    fn state_installed(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()>;
    /// Installs the working-tree file, before the state. A failure gives the
    /// state of the file with the error: a file in its initial place lets
    /// the write end with no change.
    fn install_file(
        &mut self,
        live: &ProvenanceLayout,
    ) -> Result<InstallState, (InstallState, anyhow::Error)>;
    fn published(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()>;
    fn after_published(&mut self) -> anyhow::Result<()>;
    /// Removes the data of a write that failed and that no marker names, as
    /// the next section entry removes it after a stop.
    fn remove_unpublished(
        &mut self,
        _live: &ProvenanceLayout,
        transaction: &Utf8Path,
    ) -> anyhow::Result<()> {
        remove_unpublished(transaction)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallState {
    StateOnly,
    /// The working-tree file is not in its initial place: the replacement is
    /// there, or the backup holds the initial file. So the write goes only
    /// forward: a failure keeps the marker for the next section entry.
    FileMoved,
}

/// Lands `writes` on the state directory of `live`, with the working-tree
/// file of `hook`. The cost is proportional to the changed bytes.
#[rule("rule_live_projection_shows_only_published_state")]
pub fn land(
    live: &ProvenanceLayout,
    writes: &WriteSet,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    let transaction_name = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let transaction = hook.kind().directory(live).join(transaction_name);
    std::fs::create_dir(&transaction)?;
    let result = stage_and_install(live, writes, &transaction, hook);
    if result.is_err() && !hook.kind().marker(live).exists() {
        // The error of the write wins. A failed removal leaves data that the
        // next section entry removes, or that stops it with its path.
        let _ = hook.remove_unpublished(live, &transaction);
    }
    result
}

/// Stages the changed files and the change list, writes the marker, and
/// then installs the files in place.
fn stage_and_install(
    live: &ProvenanceLayout,
    writes: &WriteSet,
    transaction: &Utf8Path,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    in_place::stage(live, transaction, writes)?;
    sync_directory(&hook.kind().directory(live))?;
    crate::test_probes::at("state_prepared")?;
    match hook.prepared(live, transaction) {
        Ok(()) => install_after_marker(live, transaction, hook),
        // A marker that exists names a complete change list, so the next
        // section entry completes the write.
        Err(_) if hook.kind().marker(live).exists() => Ok(()),
        Err(error) => Err(error),
    }
}

/// Installs the write that the marker names: the working-tree file, and then
/// the state. When the file install fails with the file in its initial
/// place, no file changed: the write clears its marker and fails. After that
/// point the write succeeds: when a step fails, the marker stays, and the
/// next section entry, or the next call of the storage port in this
/// section, completes the write.
#[rule("rule_write_succeeds_after_its_marker")]
fn install_after_marker(
    live: &ProvenanceLayout,
    transaction: &Utf8Path,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    match hook.install_file(live) {
        Ok(_) => {}
        Err((InstallState::StateOnly, error)) => {
            clear_marker(live, hook.kind()).map_err(crate::write_error::publication_started)?;
            return Err(error);
        }
        Err((InstallState::FileMoved, _)) => return Ok(()),
    }
    let installed = in_place::install(live, transaction)
        .and_then(|()| hook.state_installed(live))
        .and_then(|()| hook.published(live, transaction))
        .and_then(|()| hook.after_published());
    if installed.is_ok() {
        end_published_write(live, transaction, hook);
    }
    Ok(())
}

/// Removes the transaction data and the marker of a write that published.
/// A failure of that removal keeps the marker for the next section entry,
/// and the write succeeds.
#[rule("rule_published_write_succeeds_when_its_cleanup_fails")]
fn end_published_write(live: &ProvenanceLayout, transaction: &Utf8Path, hook: &dyn WholeSetHook) {
    // The write published, so a failed removal does not change its answer.
    let _ = end_marked_write(live, hook.kind(), transaction);
}

/// The hook of a whole set with no working-tree file: its transaction goes
/// under `cache/import-transactions`, and the publication marker records
/// each phase.
pub struct StatePublication;

impl WholeSetHook for StatePublication {
    fn kind(&self) -> TransactionKind {
        TransactionKind::Import
    }

    fn prepared(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        write_publication_marker(live, transaction, PublicationPhase::Installing)?;
        crate::test_probes::at("state_marker_prepared")
    }

    fn state_installed(&mut self, _live: &ProvenanceLayout) -> anyhow::Result<()> {
        Ok(())
    }

    fn install_file(
        &mut self,
        _live: &ProvenanceLayout,
    ) -> Result<InstallState, (InstallState, anyhow::Error)> {
        Ok(InstallState::StateOnly)
    }

    fn published(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        write_publication_marker(live, transaction, PublicationPhase::Published)
    }

    fn after_published(&mut self) -> anyhow::Result<()> {
        crate::test_probes::at("state_published")
    }
}
