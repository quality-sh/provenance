use super::export::ScopeExport;
use crate::output;
use crate::store::{ScopeSnapshot, Store};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, CONTRIBUTION_KIND,
    SYNTHESIS_KIND,
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
    let records = exported.sources.len()
        + exported.domains.len()
        + exported.requirements.len()
        + exported.boundaries.len()
        + exported.topics.len()
        + exported.questions.len()
        + exported.resolutions.len()
        + exported.rules.len()
        + exported.implementation_bindings.len()
        + exported.threads.len()
        + exported.messages.len()
        + exported.contributions.len()
        + exported.synthesis_packets.len()
        + exported.proposal_cards.len()
        + exported.assertion_records.len()
        + exported.dispositions.len();
    let store = Store::open(repo);
    store.with_repository_publication(|| {
        store.ensure_review_portable(&scope_id)?;
        anyhow::ensure!(
            exported
                .requirements
                .iter()
                .all(|r| r.schema_version == provenance_core::SUPPORTED_SCHEMA_VERSION),
            "import cannot restore enrolled Requirements without their review history"
        );
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
        let stored = store.snapshot(&scope_id)?;
        ensure_immutable_records_preserved(
            "proposal",
            &stored.proposal_cards,
            &exported.proposal_cards,
            |record| record.id.as_str(),
        )?;
        ensure_immutable_records_preserved(
            "assertion",
            &stored.assertion_records,
            &exported.assertion_records,
            |record| record.id.as_str(),
        )?;
        ensure_immutable_records_preserved(
            "disposition",
            &stored.dispositions,
            &exported.dispositions,
            |record| record.id.as_str(),
        )?;
        ensure_asserted_evidence_preserved(&stored, &exported)?;
        apply_import(store.layout(), &scope_id, &exported, dry_run)
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
    stored: &ScopeSnapshot,
    incoming: &ScopeExport,
) -> anyhow::Result<()> {
    for existing in &stored.contributions {
        match incoming
            .contributions
            .iter()
            .find(|record| record.id == existing.id)
        {
            Some(replacement) => {
                ensure_asserted_contribution_unchanged(
                    existing,
                    replacement,
                    &stored.assertion_records,
                )?;
            }
            None => ensure_asserted_evidence_not_deleted(
                CONTRIBUTION_KIND,
                existing.id.as_str(),
                assertion_cites_contribution(existing, &stored.assertion_records),
            )?,
        }
    }
    for existing in &stored.synthesis_packets {
        match incoming
            .synthesis_packets
            .iter()
            .find(|record| record.id == existing.id)
        {
            Some(replacement) => {
                ensure_asserted_synthesis_unchanged(
                    existing,
                    replacement,
                    &stored.assertion_records,
                )?;
            }
            None => ensure_asserted_evidence_not_deleted(
                SYNTHESIS_KIND,
                existing.id.as_str(),
                assertion_cites_synthesis(existing, &stored.assertion_records),
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
    provenance_store::publication::with_staged_state(live_layout, dry_run, |layout| {
        let staged_scope = layout.scopes_dir().join(scope_id.as_str());
        if staged_scope.exists() {
            std::fs::remove_dir_all(&staged_scope)?;
        }
        write_scope(layout, scope_id, exported)?;
        let staged_repo = layout.provenance_dir().parent().unwrap().to_path_buf();
        super::check::validate_repository(staged_repo)?;
        ensure_changed_statements_are_clean(live_layout, layout, scope_id)
    })
}

#[rule("rule_ste_import_changed_statement_gate")]
fn ensure_changed_statements_are_clean(
    live_layout: &ProvenanceLayout,
    staged_layout: &ProvenanceLayout,
    scope_id: &ScopeId,
) -> anyhow::Result<()> {
    let live = Store::open(live_layout.provenance_dir().parent().unwrap());
    let staged = Store::open(staged_layout.provenance_dir().parent().unwrap());
    let live_snapshot = live.snapshot(scope_id)?;
    let staged_snapshot = staged.snapshot(scope_id)?;
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(live_layout);
    let diagnostics = analyze_changed_statements(
        &live_snapshot.requirements,
        &live_snapshot.rules,
        &staged_snapshot.requirements,
        &staged_snapshot.rules,
        dictionary.as_ref(),
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(violation_error(&diagnostics))
    }
}

pub(super) fn handle(
    repo: Utf8PathBuf,
    scope: String,
    input: Utf8PathBuf,
    dry_run: bool,
) -> anyhow::Result<()> {
    let report = import_scope(repo, scope, input, dry_run)?;
    output::print_json(&report)?;
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
