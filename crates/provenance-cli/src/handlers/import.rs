use super::export::ScopeExport;
use crate::output;
use crate::store::{ScopeSnapshot, Store};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, ScopeShards,
    StateStore, CONTRIBUTION_KIND, SYNTHESIS_KIND,
};
use provenance_store::statement_analysis::{analyze_changed_statements, violation_error};
use serde::Serialize;

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

/// Reads one export document, refusing record fields a typed import drops.
///
/// The export envelope refuses unknown fields on its own. A record type is
/// open, so without this check an unknown field on one record would be
/// dropped when the scope is written back, and the published state would
/// quietly miss data the document carried. The refusal happens before the
/// publication lock is taken, so a refused import changes nothing.
fn deserialize_scope_export(input: &str) -> anyhow::Result<ScopeExport> {
    let mut unknown = None;
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let decoded = serde_ignored::deserialize(&mut deserializer, |path| {
        if unknown.is_none() {
            unknown = Some(path.to_string());
        }
    });
    match decoded {
        Ok(exported) => match unknown {
            Some(field) => anyhow::bail!(
                "import refuses unknown field `{field}`: writing it back would drop it"
            ),
            None => {
                deserializer.end()?;
                Ok(exported)
            }
        },
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
        StateStore::new(layout.clone()).import_scope(scope_id, &scope_shards(exported))?;
        let staged_repo = layout.provenance_dir().parent().unwrap().to_path_buf();
        super::check::validate_repository(staged_repo)?;
        ensure_changed_statements_are_clean(live_layout, layout, scope_id)
    })
}

fn scope_shards(exported: &ScopeExport) -> ScopeShards<'_> {
    ScopeShards {
        sources: &exported.sources,
        domains: &exported.domains,
        requirements: &exported.requirements,
        boundaries: &exported.boundaries,
        topics: &exported.topics,
        questions: &exported.questions,
        resolutions: &exported.resolutions,
        rules: &exported.rules,
        verification_bindings: &exported.verification_bindings,
        implementation_bindings: &exported.implementation_bindings,
        threads: &exported.threads,
        messages: &exported.messages,
        contributions: &exported.contributions,
        synthesis_packets: &exported.synthesis_packets,
        proposal_cards: &exported.proposal_cards,
        assertion_records: &exported.assertion_records,
        dispositions: &exported.dispositions,
    }
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
    use super::{
        deserialize_scope_export, ensure_asserted_evidence_not_deleted, CONTRIBUTION_KIND,
        SYNTHESIS_KIND,
    };
    use provenance_macros::verifies;
    use serde_json::json;

    fn minimal_export(source: &serde_json::Value) -> serde_json::Value {
        json!({
            "scope": "default",
            "sources": [source],
            "requirements": [],
            "resolutions": [],
            "rules": [],
            "threads": [],
            "messages": []
        })
    }

    #[test]
    fn import_refuses_an_unknown_record_field_instead_of_dropping_it() {
        let document = minimal_export(&json!({
            "schema_version": 2,
            "scope_id": "default",
            "id": "source_one",
            "name": "Policy",
            "source_type": "policy",
            "url": null,
            "extension": {"owner": "newer-tool"}
        }));

        let message = match deserialize_scope_export(&document.to_string()) {
            Ok(_) => panic!("import accepted a record field that it cannot represent"),
            Err(error) => error.to_string(),
        };

        assert!(
            message.contains("unknown field `sources.0.extension`"),
            "{message}"
        );
    }

    #[test]
    fn import_accepts_a_supported_record_alias() {
        let document = minimal_export(&json!({
            "schema_version": 2,
            "scope_id": "default",
            "id": "source_one",
            "name": "Policy",
            "sourceType": "policy",
            "url": null
        }));

        let exported = deserialize_scope_export(&document.to_string()).unwrap();

        assert_eq!(exported.sources[0].source_type.as_str(), "policy");
    }

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
