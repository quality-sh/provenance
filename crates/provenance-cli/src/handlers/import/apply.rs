//! Staged whole-state apply and publication swap for scope import, split
//! out of the import handler: the transaction directory, the staged apply,
//! the marker-guarded swap into place, and the rollback path.

use super::scope_writer::write_scope;
use crate::handlers::export::ScopeExport;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::StateStore;

pub(super) fn apply_import(
    live_layout: &ProvenanceLayout,
    scope_id: &provenance_core::ScopeId,
    exported: &ScopeExport,
    dry_run: bool,
) -> anyhow::Result<()> {
    let transaction_name = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let transaction = create_import_transaction(live_layout, &transaction_name)?;
    let _cleanup = TransactionCleanup::new(transaction.clone(), live_layout);
    let staged_repo = transaction.join("staged-repo");
    copy_directory(
        &live_layout.state_dir(),
        &ProvenanceLayout::new(staged_repo.clone()).state_dir(),
    )?;
    let layout = ProvenanceLayout::new(staged_repo.clone());
    let staged_scope = layout.scopes_dir().join(scope_id.as_str());
    if staged_scope.exists() {
        std::fs::remove_dir_all(&staged_scope)
            .map_err(|error| anyhow::anyhow!("clear staged scope {staged_scope}: {error}"))?;
    }
    write_scope(&layout, scope_id, exported)?;
    super::super::check::validate_repository(staged_repo)?;
    ensure_changed_statements_are_clean(live_layout, &layout, scope_id)?;
    if !dry_run {
        provenance_store::publication::sync_tree(&layout.state_dir())?;
        let backup = transaction.join("backup-state");
        provenance_store::publication::write_publication_marker(
            live_layout,
            &transaction,
            provenance_store::publication::PublicationPhase::Prepared,
        )?;
        std::fs::rename(live_layout.state_dir(), &backup).map_err(|error| {
            anyhow::anyhow!(
                "move live state {} to backup: {error}",
                live_layout.state_dir()
            )
        })?;
        if let Err(error) =
            provenance_store::publication::sync_directory(&live_layout.provenance_dir())
                .and_then(|()| {
                    provenance_store::publication::write_publication_marker(
                        live_layout,
                        &transaction,
                        provenance_store::publication::PublicationPhase::BackupCreated,
                    )
                })
                .and_then(|()| {
                    std::fs::rename(layout.state_dir(), live_layout.state_dir()).map_err(|error| {
                        anyhow::anyhow!("install staged state {}: {error}", layout.state_dir())
                    })
                })
                .and_then(|()| {
                    provenance_store::publication::sync_directory(&live_layout.provenance_dir())
                })
                .and_then(|()| {
                    provenance_store::publication::write_publication_marker(
                        live_layout,
                        &transaction,
                        provenance_store::publication::PublicationPhase::Published,
                    )
                })
        {
            rollback_publication(live_layout, &layout, &backup)?;
            return Err(error);
        }
        if std::fs::remove_dir_all(&transaction).is_ok() {
            let _ = provenance_store::publication::clear_publication_marker(live_layout);
        }
        return Ok(());
    }
    std::fs::remove_dir_all(&transaction)
        .map_err(|error| anyhow::anyhow!("remove import transaction {transaction}: {error}"))?;
    Ok(())
}

#[provenance_macros::rule("rule_ste_import_changed_statement_gate")]
fn ensure_changed_statements_are_clean(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    scope_id: &provenance_core::ScopeId,
) -> anyhow::Result<()> {
    let live = StateStore::new(live_layout.clone());
    let staged = StateStore::new(staged_layout.clone());
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(live_layout);
    let diagnostics = provenance_store::statement_analysis::analyze_changed_statements(
        &live.list_requirements(scope_id)?,
        &live.list_rules(scope_id)?,
        &staged.list_requirements(scope_id)?,
        &staged.list_rules(scope_id)?,
        dictionary.as_ref(),
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(provenance_store::statement_analysis::violation_error(
            &diagnostics,
        ))
    }
}

fn create_import_transaction(
    layout: &ProvenanceLayout,
    transaction_name: &str,
) -> anyhow::Result<Utf8PathBuf> {
    let transactions = layout.import_transactions_dir();
    let transaction = transactions.join(transaction_name);
    std::fs::create_dir(&transaction)?;
    Ok(transaction)
}

struct TransactionCleanup {
    transaction: Utf8PathBuf,
    publication_marker: Utf8PathBuf,
}

impl TransactionCleanup {
    fn new(transaction: Utf8PathBuf, live_layout: &ProvenanceLayout) -> Self {
        Self {
            transaction,
            publication_marker: live_layout.publication_marker_path(),
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
    backup: &Utf8Path,
) -> anyhow::Result<()> {
    if live_layout.state_dir().exists() {
        std::fs::rename(live_layout.state_dir(), staged_layout.state_dir()).map_err(|error| {
            anyhow::anyhow!("return live state to stage during rollback: {error}")
        })?;
    }
    if backup.exists() {
        std::fs::rename(backup, live_layout.state_dir())
            .map_err(|error| anyhow::anyhow!("restore backup state during rollback: {error}"))?;
    }
    provenance_store::publication::sync_directory(&live_layout.provenance_dir())?;
    provenance_store::publication::clear_publication_marker(live_layout)
}

fn copy_directory(source: &Utf8Path, destination: &Utf8Path) -> anyhow::Result<()> {
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
