use super::export::ScopeExport;
use crate::output::{self, OutputFormat};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, StateStore,
    CONTRIBUTION_KIND, SYNTHESIS_KIND,
};
use provenance_store::statement_analysis::{analyze_changed_statements, violation_error};
use serde::Serialize;

mod scope_writer;

use scope_writer::write_scope;

#[derive(Serialize)]
pub struct ImportReport {
    pub status: &'static str,
    pub dry_run: bool,
    pub records: usize,
}

pub(super) fn import_scope(
    repo: Utf8PathBuf,
    scope: String,
    input: Utf8PathBuf,
    dry_run: bool,
) -> anyhow::Result<ImportReport> {
    let input = std::fs::read_to_string(input)?;
    let exported = deserialize_scope_export(&input)?;
    anyhow::ensure!(
        exported.scope == scope,
        "import scope does not match --scope"
    );
    let scope_id = ScopeId::new(scope)?;
    anyhow::ensure!(
        exported.edges.iter().all(|edge| edge.scope_id == scope_id),
        "edge scope_id must match import scope"
    );
    let records = exported.sources.len()
        + exported.domains.len()
        + exported.requirements.len()
        + exported.boundaries.len()
        + exported.topics.len()
        + exported.questions.len()
        + exported.resolutions.len()
        + exported.rules.len()
        + exported.implementation_bindings.len()
        + exported.edges.len()
        + exported.threads.len()
        + exported.messages.len()
        + exported.contributions.len()
        + exported.synthesis_packets.len()
        + exported.proposal_cards.len()
        + exported.assertion_records.len()
        + exported.dispositions.len();
    let live_layout = ProvenanceLayout::new(repo);
    provenance_store::publication::with_repository_publication(&live_layout, || {
        let store = StateStore::new(live_layout.clone());
        let manifest = store.manifest()?;
        provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
            legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
            disposition_actor_ids: &manifest.disposition_actor_ids,
            contributions: &exported.contributions,
            synthesis_packets: &exported.synthesis_packets,
            proposals: &exported.proposal_cards,
            assertions: &exported.assertion_records,
            dispositions: &exported.dispositions,
        })?;
        ensure_immutable_records_preserved(
            "proposal",
            &store.list_proposal_definitions(&scope_id)?,
            &exported.proposal_cards,
            |record| record.id.as_str(),
        )?;
        ensure_immutable_records_preserved(
            "assertion",
            &store.list_assertion_records(&scope_id)?,
            &exported.assertion_records,
            |record| record.id.as_str(),
        )?;
        ensure_immutable_records_preserved(
            "disposition",
            &store.list_dispositions(&scope_id)?,
            &exported.dispositions,
            |record| record.id.as_str(),
        )?;
        ensure_asserted_evidence_preserved(&store, &scope_id, &exported)?;
        apply_import(&live_layout, &scope_id, &exported, dry_run)
    })?;
    Ok(ImportReport {
        status: "ok",
        dry_run,
        records,
    })
}

fn deserialize_scope_export(input: &str) -> anyhow::Result<ScopeExport> {
    match serde_json::from_str(input) {
        Ok(exported) => Ok(exported),
        Err(_) if has_removed_service_family(input) => anyhow::bail!(
            "this export predates the service family removal; re-export from current provenance"
        ),
        Err(error) => Err(error.into()),
    }
}

fn has_removed_service_family(input: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(input).is_ok_and(|value| {
        value.as_object().is_some_and(|object| {
            object.contains_key("services") || object.contains_key("service_bindings")
        })
    })
}

fn ensure_immutable_records_preserved<T: Serialize>(
    kind: &str,
    existing: &[T],
    incoming: &[T],
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    for current in existing {
        let record_id = id(current);
        let replacement = incoming
            .iter()
            .find(|record| id(record) == record_id)
            .ok_or_else(|| {
                anyhow::anyhow!("immutable {kind} {record_id} must be preserved by import")
            })?;
        anyhow::ensure!(
            serde_json::to_value(current)? == serde_json::to_value(replacement)?,
            "immutable {kind} {record_id} must remain unchanged"
        );
    }
    Ok(())
}

/// Evidence an assertion rests on cannot be dropped.
///
/// Import is the only path that can drop a record at all: it stands a whole
/// scope in place of the stored one, so a record the incoming scope never
/// mentions is gone. A record no assertion cites may be dropped freely.
///
/// The caller reads "cited by an assertion" with `provenance-store`'s reading
/// for the record's kind, so this and the freeze answer the same question of
/// the same record.
#[rule("rule_asserted_evidence_undeletable")]
fn ensure_asserted_evidence_not_deleted(
    kind: &str,
    id: &str,
    cited_by_assertion: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cited_by_assertion,
        "{kind} {id} is referenced by an assertion and cannot be deleted"
    );
    Ok(())
}

/// Both halves of the freeze over the evidence an import replaces: a record
/// the incoming scope carries is judged by the store's freeze, and a record it
/// omits by the deletion rule above.
fn ensure_asserted_evidence_preserved(
    store: &StateStore,
    scope_id: &ScopeId,
    incoming: &ScopeExport,
) -> anyhow::Result<()> {
    let assertions = store.list_assertion_records(scope_id)?;
    for existing in store.list_contributions(scope_id)? {
        match incoming
            .contributions
            .iter()
            .find(|record| record.id == existing.id)
        {
            Some(replacement) => {
                ensure_asserted_contribution_unchanged(&existing, replacement, &assertions)?;
            }
            None => ensure_asserted_evidence_not_deleted(
                CONTRIBUTION_KIND,
                existing.id.as_str(),
                assertion_cites_contribution(&existing, &assertions),
            )?,
        }
    }
    for existing in store.list_synthesis_packets(scope_id)? {
        match incoming
            .synthesis_packets
            .iter()
            .find(|record| record.id == existing.id)
        {
            Some(replacement) => {
                ensure_asserted_synthesis_unchanged(&existing, replacement, &assertions)?;
            }
            None => ensure_asserted_evidence_not_deleted(
                SYNTHESIS_KIND,
                existing.id.as_str(),
                assertion_cites_synthesis(&existing, &assertions),
            )?,
        }
    }
    Ok(())
}

fn apply_import(
    live_layout: &ProvenanceLayout,
    scope_id: &ScopeId,
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
    super::check::validate_repository(staged_repo)?;
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

#[rule("rule_ste_import_changed_statement_gate")]
fn ensure_changed_statements_are_clean(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    scope_id: &ScopeId,
) -> anyhow::Result<()> {
    let live = StateStore::new(live_layout.clone());
    let staged = StateStore::new(staged_layout.clone());
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(live_layout);
    let diagnostics = analyze_changed_statements(
        &live.list_requirements(scope_id)?,
        &live.list_rules(scope_id)?,
        &staged.list_requirements(scope_id)?,
        &staged.list_rules(scope_id)?,
        dictionary.as_ref(),
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(violation_error(&diagnostics))
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
    backup: &camino::Utf8Path,
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

fn copy_directory(source: &camino::Utf8Path, destination: &camino::Utf8Path) -> anyhow::Result<()> {
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

pub(super) fn handle(
    repo: Utf8PathBuf,
    scope: String,
    input: Utf8PathBuf,
    dry_run: bool,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let report = import_scope(repo, scope, input, dry_run)?;
    output::print(format, &report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_asserted_evidence_not_deleted, CONTRIBUTION_KIND, SYNTHESIS_KIND};
    use provenance_macros::verifies;

    // The decision ranges over two finite axes and nothing else: the kind of
    // evidence being dropped, and whether an assertion cites it. The kinds are
    // the two `provenance-store` freezes, named from the store so a third kind
    // has to be spelled here before it can be dropped silently.
    const EVIDENCE_KINDS: [(&str, &str); 2] = [
        (CONTRIBUTION_KIND, "contribution_a"),
        (SYNTHESIS_KIND, "synthesis_a"),
    ];

    #[test]
    #[verifies("rule_asserted_evidence_undeletable", exhaustion)]
    fn evidence_may_be_dropped_exactly_when_no_assertion_cites_it() {
        for (kind, id) in EVIDENCE_KINDS {
            for cited_by_assertion in [false, true] {
                let outcome = ensure_asserted_evidence_not_deleted(kind, id, cited_by_assertion);

                // Independent restatement of the decision: an assertion's
                // ground stays where it is, and everything else may go.
                assert_eq!(
                    outcome.is_ok(),
                    !cited_by_assertion,
                    "{kind} {id} cited={cited_by_assertion}"
                );
            }
        }
    }

    #[test]
    #[verifies("rule_asserted_evidence_undeletable", examples)]
    fn the_refusal_names_the_record_that_may_not_go() {
        for (kind, id) in EVIDENCE_KINDS {
            let error = ensure_asserted_evidence_not_deleted(kind, id, true)
                .unwrap_err()
                .to_string();
            assert_eq!(
                error,
                format!("{kind} {id} is referenced by an assertion and cannot be deleted")
            );
        }
    }
}
