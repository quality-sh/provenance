//! Complete-state publication shared by import and review saves.
use super::{
    clear_publication_marker, sync_directory, sync_tree, with_repository_publication,
    write_publication_marker, PublicationPhase,
};
use crate::layout::ProvenanceLayout;
use camino::Utf8PathBuf;

/// Copies the complete state tree and publishes the prepared result under one lock.
/// Copy cost is proportional to all state bytes, including existing evidence.
pub fn with_staged_state<R>(
    live_layout: &ProvenanceLayout,
    dry_run: bool,
    prepare: impl FnOnce(&ProvenanceLayout) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    with_repository_publication(live_layout, || {
        let mut publication = ImportPublication;
        stage_with_hook(live_layout, dry_run, prepare, &mut publication).map_err(|error| {
            if live_layout.publication_marker_path().exists() {
                crate::write_error::publication_started(error)
            } else {
                // A failed staged write or completed rollback did not commit live state.
                match error.downcast::<crate::write_error::PublicationStarted>() {
                    Ok(staged) => staged.0,
                    Err(error) => error,
                }
            }
        })
    })
}

pub(super) trait StagedStateHook {
    fn transactions_dir(&self, live: &ProvenanceLayout) -> Utf8PathBuf;
    fn marker_path(&self, live: &ProvenanceLayout) -> Utf8PathBuf;
    fn prepared(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()>;
    fn backup_created(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()>;
    fn state_installed(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()>;
    fn install_file(&mut self) -> anyhow::Result<InstallState>;
    fn published(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()>;
    fn after_published(&mut self) -> anyhow::Result<()>;
    fn rollback_finished(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()>;
    fn finish(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InstallState {
    StateOnly,
    FileInstalled,
}

pub(super) fn stage_with_hook<R>(
    live_layout: &ProvenanceLayout,
    dry_run: bool,
    prepare: impl FnOnce(&ProvenanceLayout) -> anyhow::Result<R>,
    hook: &mut impl StagedStateHook,
) -> anyhow::Result<R> {
    let transaction_name = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let transaction = hook.transactions_dir(live_layout).join(transaction_name);
    std::fs::create_dir(&transaction)?;
    let _cleanup = TransactionCleanup::new(transaction.clone(), hook.marker_path(live_layout));
    let staged_repo = transaction.join("staged-repo");
    copy_directory(
        &live_layout.state_dir(),
        &ProvenanceLayout::new(staged_repo.clone()).state_dir(),
    )?;
    let layout = ProvenanceLayout::new(staged_repo.clone());
    let result = prepare(&layout)?;
    crate::test_probes::at("state_prepared")?;
    if !dry_run {
        publish_staged_state(live_layout, &layout, &staged_repo, &transaction, hook)?;
        return Ok(result);
    }
    std::fs::remove_dir_all(&transaction)
        .map_err(|error| anyhow::anyhow!("remove import transaction {transaction}: {error}"))?;
    Ok(result)
}

fn publish_staged_state(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    staged_repo: &camino::Utf8Path,
    transaction: &camino::Utf8Path,
    hook: &mut impl StagedStateHook,
) -> anyhow::Result<()> {
    sync_tree(&staged_layout.state_dir())?;
    sync_directory(&staged_layout.provenance_dir())?;
    sync_directory(staged_repo)?;
    sync_directory(transaction)?;
    sync_directory(&hook.transactions_dir(live_layout))?;
    let backup = transaction.join("backup-state");
    hook.prepared(live_layout, transaction)?;
    std::fs::rename(live_layout.state_dir(), &backup).map_err(|error| {
        anyhow::anyhow!(
            "move live state {} to backup: {error}",
            live_layout.state_dir()
        )
    })?;
    install_staged_state(live_layout, staged_layout, transaction, &backup, hook)?;
    let install_state = install_file(live_layout, staged_layout, &backup, hook)?;
    record_publication(
        live_layout,
        staged_layout,
        transaction,
        &backup,
        install_state,
        hook,
    )?;
    hook.after_published()?;
    hook.finish(live_layout, transaction)
}

fn install_staged_state(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    transaction: &camino::Utf8Path,
    backup: &camino::Utf8Path,
    hook: &mut impl StagedStateHook,
) -> anyhow::Result<()> {
    let state_result = crate::test_probes::at("state_after_backup_rename")
        .and_then(|()| hook.backup_created(live_layout, transaction))
        .and_then(|()| crate::test_probes::at("state_before_install"))
        .and_then(|()| {
            std::fs::rename(staged_layout.state_dir(), live_layout.state_dir()).map_err(|error| {
                anyhow::anyhow!(
                    "install staged state {}: {error}",
                    staged_layout.state_dir()
                )
            })
        })
        .and_then(|()| crate::test_probes::at("state_after_install_rename"))
        .and_then(|()| crate::test_probes::at("state_installed"))
        .and_then(|()| sync_directory(&live_layout.provenance_dir()))
        .and_then(|()| hook.state_installed(live_layout));
    if let Err(error) = state_result {
        rollback_after_error(live_layout, staged_layout, backup, hook)?;
        return Err(error);
    }
    Ok(())
}

fn install_file(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    backup: &camino::Utf8Path,
    hook: &mut impl StagedStateHook,
) -> anyhow::Result<InstallState> {
    match hook.install_file() {
        Ok(state) => Ok(state),
        Err(error) => {
            rollback_after_error(live_layout, staged_layout, backup, hook)?;
            Err(error)
        }
    }
}

fn record_publication(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    transaction: &camino::Utf8Path,
    backup: &camino::Utf8Path,
    install_state: InstallState,
    hook: &mut impl StagedStateHook,
) -> anyhow::Result<()> {
    let Err(error) = hook.published(live_layout, transaction) else {
        return Ok(());
    };
    if install_state == InstallState::FileInstalled {
        return Err(error);
    }
    rollback_after_error(live_layout, staged_layout, backup, hook)?;
    Err(error)
}

fn rollback_after_error(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    backup: &camino::Utf8Path,
    hook: &mut impl StagedStateHook,
) -> anyhow::Result<()> {
    rollback_publication(live_layout, staged_layout, backup)
        .map_err(crate::write_error::publication_started)?;
    hook.rollback_finished(live_layout)
        .map_err(crate::write_error::publication_started)
}

struct TransactionCleanup {
    transaction: Utf8PathBuf,
    publication_marker: Utf8PathBuf,
}

impl TransactionCleanup {
    const fn new(transaction: Utf8PathBuf, publication_marker: Utf8PathBuf) -> Self {
        Self {
            transaction,
            publication_marker,
        }
    }
}

impl Drop for TransactionCleanup {
    fn drop(&mut self) {
        if !self.publication_marker.exists() && self.transaction.exists() {
            let _ = std::fs::remove_dir_all(&self.transaction);
        }
    }
}

fn rollback_publication(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    backup: &camino::Utf8Path,
) -> anyhow::Result<()> {
    crate::test_probes::at("state_before_rollback")?;
    if live_layout.state_dir().exists() {
        std::fs::rename(live_layout.state_dir(), staged_layout.state_dir()).map_err(|error| {
            anyhow::anyhow!("return live state to stage during rollback: {error}")
        })?;
    }
    if backup.exists() {
        std::fs::rename(backup, live_layout.state_dir())
            .map_err(|error| anyhow::anyhow!("restore backup state during rollback: {error}"))?;
    }
    sync_directory(&live_layout.provenance_dir())
}

struct ImportPublication;

impl StagedStateHook for ImportPublication {
    fn transactions_dir(&self, live: &ProvenanceLayout) -> Utf8PathBuf {
        live.import_transactions_dir()
    }

    fn marker_path(&self, live: &ProvenanceLayout) -> Utf8PathBuf {
        live.publication_marker_path()
    }

    fn prepared(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()> {
        write_publication_marker(live, transaction, PublicationPhase::Prepared)?;
        crate::test_probes::at("state_marker_prepared")
    }

    fn backup_created(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
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

    fn published(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()> {
        write_publication_marker(live, transaction, PublicationPhase::Published)
    }

    fn after_published(&mut self) -> anyhow::Result<()> {
        crate::test_probes::at("state_published")
    }

    fn rollback_finished(&mut self, live: &ProvenanceLayout) -> anyhow::Result<()> {
        clear_publication_marker(live)
    }

    fn finish(
        &mut self,
        live: &ProvenanceLayout,
        transaction: &camino::Utf8Path,
    ) -> anyhow::Result<()> {
        if std::fs::remove_dir_all(transaction).is_ok() {
            let _ = clear_publication_marker(live);
        }
        Ok(())
    }
}

pub(super) fn copy_directory(
    source: &camino::Utf8Path,
    destination: &camino::Utf8Path,
) -> anyhow::Result<()> {
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
