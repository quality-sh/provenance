//! A whole set of many keys on local disk: a copy of the state directory
//! with the set applied, swapped in for the live state directory.
//!
//! The transaction lives under `.provenance/cache`. A marker names its phase,
//! so recovery at the next section entry completes or rolls back a swap that
//! a crash interrupted.

use super::durability::{sync_directory, sync_tree};
use super::recovery::{clear_publication_marker, write_publication_marker, PublicationPhase};
use crate::layout::ProvenanceLayout;
use crate::storage::WriteSet;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;

/// The steps that a publication names in one whole-set transaction.
///
/// The steps say where the transaction and its marker live, what each phase
/// records, and which working-tree file lands with the state, if any.
pub trait WholeSetHook {
    fn transactions_dir(&self, live: &ProvenanceLayout) -> Utf8PathBuf;
    fn marker_path(&self, live: &ProvenanceLayout) -> Utf8PathBuf;
    fn prepared(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()>;
    fn backup_created(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &Utf8Path,
    ) -> anyhow::Result<()>;
    fn state_installed(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()>;
    fn install_file(&mut self) -> anyhow::Result<InstallState>;
    fn published(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()>;
    fn after_published(&mut self) -> anyhow::Result<()>;
    fn rollback_finished(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()>;
    fn finish(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallState {
    StateOnly,
    FileInstalled,
}

/// Lands `writes` on the state directory of `live` as one swap. The copy
/// cost is proportional to all state bytes.
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
    let transaction = hook.transactions_dir(live).join(transaction_name);
    std::fs::create_dir(&transaction)?;
    let _cleanup = TransactionCleanup {
        transaction: transaction.clone(),
        publication_marker: hook.marker_path(live),
    };
    let staged_repo = transaction.join("staged-repo");
    let staged = ProvenanceLayout::new(staged_repo.clone());
    copy_directory(&live.state_dir(), &staged.state_dir())?;
    apply(&staged, writes)?;
    crate::test_probes::at("state_prepared")?;
    publish_staged_state(live, &staged, &staged_repo, &transaction, hook)
}

/// Applies `writes` to the copy of the state directory at `staged`.
fn apply(staged: &ProvenanceLayout, writes: &WriteSet) -> anyhow::Result<()> {
    let state = staged.state_dir();
    for key in writes.deletes() {
        let path = state.join(key.as_str());
        match std::fs::remove_file(crate::long_path::extended(&path)?) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(anyhow::Error::new(error).context(format!("remove {path}")));
            }
            _ => {}
        }
    }
    for (key, bytes) in writes.puts() {
        super::write_file_atomic(&state.join(key.as_str()), bytes)?;
    }
    Ok(())
}

fn publish_staged_state(
    live: &ProvenanceLayout,
    staged: &ProvenanceLayout,
    staged_repo: &Utf8Path,
    transaction: &Utf8Path,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    sync_tree(&staged.state_dir())?;
    sync_directory(&staged.provenance_dir())?;
    sync_directory(staged_repo)?;
    sync_directory(transaction)?;
    sync_directory(&hook.transactions_dir(live))?;
    let backup = transaction.join("backup-state");
    hook.prepared(live, transaction)?;
    std::fs::rename(live.state_dir(), &backup).map_err(|error| {
        anyhow::anyhow!("move live state {} to backup: {error}", live.state_dir())
    })?;
    install_staged_state(live, staged, transaction, &backup, hook)?;
    let install_state = install_file(live, staged, &backup, hook)?;
    record_publication(live, staged, transaction, &backup, install_state, hook)?;
    hook.after_published()?;
    hook.finish(live, transaction)
}

fn install_staged_state(
    live: &ProvenanceLayout,
    staged: &ProvenanceLayout,
    transaction: &Utf8Path,
    backup: &Utf8Path,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    let state_result = crate::test_probes::at("state_after_backup_rename")
        .and_then(|()| hook.backup_created(live, transaction))
        .and_then(|()| crate::test_probes::at("state_before_install"))
        .and_then(|()| {
            std::fs::rename(staged.state_dir(), live.state_dir()).map_err(|error| {
                anyhow::anyhow!("install staged state {}: {error}", staged.state_dir())
            })
        })
        .and_then(|()| crate::test_probes::at("state_after_install_rename"))
        .and_then(|()| crate::test_probes::at("state_installed"))
        .and_then(|()| sync_directory(&live.provenance_dir()))
        .and_then(|()| hook.state_installed(live));
    if let Err(error) = state_result {
        rollback_after_error(live, staged, backup, hook)?;
        return Err(error);
    }
    Ok(())
}

fn install_file(
    live: &ProvenanceLayout,
    staged: &ProvenanceLayout,
    backup: &Utf8Path,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<InstallState> {
    match hook.install_file() {
        Ok(state) => Ok(state),
        Err(error) => {
            rollback_after_error(live, staged, backup, hook)?;
            Err(error)
        }
    }
}

fn record_publication(
    live: &ProvenanceLayout,
    staged: &ProvenanceLayout,
    transaction: &Utf8Path,
    backup: &Utf8Path,
    install_state: InstallState,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    let Err(error) = hook.published(live, transaction) else {
        return Ok(());
    };
    if install_state == InstallState::FileInstalled {
        return Err(error);
    }
    rollback_after_error(live, staged, backup, hook)?;
    Err(error)
}

fn rollback_after_error(
    live: &ProvenanceLayout,
    staged: &ProvenanceLayout,
    backup: &Utf8Path,
    hook: &mut dyn WholeSetHook,
) -> anyhow::Result<()> {
    rollback_publication(live, staged, backup).map_err(crate::write_error::publication_started)?;
    hook.rollback_finished(live)
        .map_err(crate::write_error::publication_started)
}

/// Removes the transaction when it ends with no marker, also on unwind.
struct TransactionCleanup {
    transaction: Utf8PathBuf,
    publication_marker: Utf8PathBuf,
}

impl Drop for TransactionCleanup {
    fn drop(&mut self) {
        if !self.publication_marker.exists() && self.transaction.exists() {
            let _ = std::fs::remove_dir_all(&self.transaction);
        }
    }
}

fn rollback_publication(
    live: &ProvenanceLayout,
    staged: &ProvenanceLayout,
    backup: &Utf8Path,
) -> anyhow::Result<()> {
    crate::test_probes::at("state_before_rollback")?;
    if live.state_dir().exists() {
        std::fs::rename(live.state_dir(), staged.state_dir()).map_err(|error| {
            anyhow::anyhow!("return live state to stage during rollback: {error}")
        })?;
    }
    if backup.exists() {
        std::fs::rename(backup, live.state_dir())
            .map_err(|error| anyhow::anyhow!("restore backup state during rollback: {error}"))?;
    }
    sync_directory(&live.provenance_dir())
}

/// The hook of a whole set with no working-tree file: its transaction goes
/// under `cache/import-transactions`, and the publication marker records
/// each phase.
pub struct StatePublication;

impl WholeSetHook for StatePublication {
    fn transactions_dir(&self, live: &ProvenanceLayout) -> Utf8PathBuf {
        live.import_transactions_dir()
    }

    fn marker_path(&self, live: &ProvenanceLayout) -> Utf8PathBuf {
        live.publication_marker_path()
    }

    fn prepared(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        write_publication_marker(live, transaction, PublicationPhase::Prepared)?;
        crate::test_probes::at("state_marker_prepared")
    }

    fn backup_created(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &Utf8Path,
    ) -> anyhow::Result<()> {
        sync_directory(transaction)?;
        crate::test_probes::at("state_backup_created")?;
        sync_directory(&live.provenance_dir())?;
        write_publication_marker(live, transaction, PublicationPhase::BackupCreated)
    }

    fn state_installed(&mut self, _live: &ProvenanceLayout) -> anyhow::Result<()> {
        Ok(())
    }

    fn install_file(&mut self) -> anyhow::Result<InstallState> {
        Ok(InstallState::StateOnly)
    }

    fn published(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        write_publication_marker(live, transaction, PublicationPhase::Published)
    }

    fn after_published(&mut self) -> anyhow::Result<()> {
        crate::test_probes::at("state_published")
    }

    fn rollback_finished(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()> {
        clear_publication_marker(live)
    }

    fn finish(&mut self, live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
        if std::fs::remove_dir_all(transaction).is_ok() {
            let _ = clear_publication_marker(live);
        }
        Ok(())
    }
}

pub fn copy_directory(source: &Utf8Path, destination: &Utf8Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow::anyhow!("state path is not UTF-8: {}", path.display()))?;
        let target = destination.join(entry.file_name().to_string_lossy().as_ref());
        let file_type = std::fs::symlink_metadata(&source_path)?.file_type();
        if file_type.is_dir() {
            copy_directory(&source_path, &target)?;
        } else if file_type.is_file() {
            std::fs::copy(source_path, target)?;
        } else {
            anyhow::bail!("unsupported state entry: {source_path}");
        }
    }
    Ok(())
}
