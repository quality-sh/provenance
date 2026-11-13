//! The import adapter: one scope export in place of the stored scope.

use super::{RepositoryPort, ScopeExport};
use crate::repository::RepoContext;
use crate::snapshot::{snapshot, ScopeSnapshot, Store};
use camino::Utf8Path;
use provenance_core::{IdeationRecord, ScopeId};
use provenance_macros::rule;
use provenance_porcelain::import::{ImportInput, ImportOutput, ImportPort};
use provenance_porcelain::WorkFuture;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::with_publication_guard;
use provenance_store::repository::Repository;
use provenance_store::state_store::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, ScopeShards,
    StateStore,
};
use provenance_store::statement_analysis::{analyze_changed_statements, violation_error};
use serde::Serialize;

impl ImportPort for RepositoryPort {
    type Failure = anyhow::Error;

    fn import(&self, input: ImportInput) -> WorkFuture<'_, ImportOutput, anyhow::Error> {
        Box::pin(
            async move { import_scope(&self.context, Utf8Path::new(&input.input), input.dry_run) },
        )
    }
}

/// Refuses an import that changes or removes shipped-v1 terminal proposals.
fn import_scope(
    context: &RepoContext,
    input: &Utf8Path,
    dry_run: bool,
) -> anyhow::Result<ImportOutput> {
    let input = std::fs::read_to_string(input)?;
    let exported = deserialize_scope_export(&input)?;
    let repository = context.open()?;
    repository.ensure_selected(&ScopeId::new(&exported.scope)?)?;
    let scope_id = repository.scope().clone();
    let records = exported.sources.len()
        + exported.requirements.len()
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
    with_publication_guard(&repository, |guard| {
        let held = guard.repository();
        let store = Store::new(held);
        store.ensure_review_portable(&scope_id)?;
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
        let stored = store.snapshot()?;
        ensure_immutable_records_preserved(&stored.proposal_cards, &exported.proposal_cards)?;
        ensure_immutable_records_preserved(&stored.assertion_records, &exported.assertion_records)?;
        ensure_immutable_records_preserved(&stored.dispositions, &exported.dispositions)?;
        ensure_asserted_evidence_preserved(&stored, &exported)?;
        apply_import(held, &stored, &exported, dry_run)
    })?;
    Ok(ImportOutput {
        status: "ok".to_owned(),
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
        Ok(exported) => {
            if let Some(field) = unknown {
                anyhow::bail!(
                    "import refuses unknown field `{field}`: writing it back would drop it"
                );
            }
            deserializer.end()?;
            Ok(exported)
        }
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

fn ensure_immutable_records_preserved<T: Serialize + IdeationRecord>(
    existing: &[T],
    incoming: &[T],
) -> anyhow::Result<()> {
    let kind = T::KIND;
    for current in existing {
        let record_id = current.immutable_id();
        let replacement = incoming
            .iter()
            .find(|record| record.immutable_id() == record_id)
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
fn ensure_asserted_evidence_not_deleted<T: IdeationRecord>(
    existing: &T,
    cited_by_assertion: bool,
) -> anyhow::Result<()> {
    let (kind, id) = (T::KIND, existing.immutable_id());
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
            .find(|record| record.immutable_id() == existing.immutable_id())
        {
            Some(replacement) => {
                ensure_asserted_contribution_unchanged(
                    existing,
                    replacement,
                    &stored.assertion_records,
                )?;
            }
            None => ensure_asserted_evidence_not_deleted(
                existing,
                assertion_cites_contribution(existing, &stored.assertion_records),
            )?,
        }
    }
    for existing in &stored.synthesis_packets {
        match incoming
            .synthesis_packets
            .iter()
            .find(|record| record.immutable_id() == existing.immutable_id())
        {
            Some(replacement) => {
                ensure_asserted_synthesis_unchanged(
                    existing,
                    replacement,
                    &stored.assertion_records,
                )?;
            }
            None => ensure_asserted_evidence_not_deleted(
                existing,
                assertion_cites_synthesis(existing, &stored.assertion_records),
            )?,
        }
    }
    Ok(())
}

/// Publishes the export over the live state. `repository` holds the section
/// that read `live`, the snapshot before the stage; the stage body reads only
/// the staged store.
fn apply_import(
    repository: &Repository,
    live: &ScopeSnapshot,
    exported: &ScopeExport,
    dry_run: bool,
) -> anyhow::Result<()> {
    let scope_id = repository.scope();
    repository.stage(dry_run, None, |staged| {
        staged.import_scope(scope_id, &scope_shards(exported))?;
        crate::check::validate_repository(staged)?;
        ensure_changed_statements_are_clean(live, staged)
    })
}

fn scope_shards(exported: &ScopeExport) -> ScopeShards<'_> {
    ScopeShards {
        sources: &exported.sources,
        requirements: &exported.requirements,
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
    live_snapshot: &ScopeSnapshot,
    staged: &StateStore,
) -> anyhow::Result<()> {
    let identity = staged
        .repository()
        .ok_or_else(|| anyhow::anyhow!("a staged import has the live repository identity"))?;
    let staged_snapshot = snapshot(staged, identity.scope())?;
    let dictionary = provenance_store::dictionary_reference::load_project_dictionary(
        &ProvenanceLayout::new(identity.working_tree()),
    );
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

#[cfg(test)]
#[path = "../../tests/support/ideation.rs"]
mod ideation;

#[cfg(test)]
mod tests {
    use super::{deserialize_scope_export, ensure_asserted_evidence_not_deleted, ideation};
    use provenance_core::{Contribution, IdeationRecord, SynthesisPacket};
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
    // the two `provenance-store` freezes, so a third kind has to be named
    // here before it can be dropped silently.
    #[test]
    #[verifies("rule_asserted_evidence_undeletable", exhaustion)]
    fn evidence_may_be_dropped_exactly_when_no_assertion_cites_it() {
        assert_dropped_exactly_when_not_cited(&contribution());
        assert_dropped_exactly_when_not_cited(&synthesis_packet());
    }

    fn assert_dropped_exactly_when_not_cited<T: IdeationRecord>(record: &T) {
        for cited_by_assertion in [false, true] {
            let outcome = ensure_asserted_evidence_not_deleted(record, cited_by_assertion);

            // Independent restatement of the decision: an assertion's
            // ground stays where it is, and everything else may go.
            assert_eq!(
                outcome.is_ok(),
                !cited_by_assertion,
                "{} {} cited={cited_by_assertion}",
                T::KIND,
                record.immutable_id()
            );
        }
    }

    #[test]
    #[verifies("rule_asserted_evidence_undeletable", examples)]
    fn the_refusal_names_the_record_that_may_not_go() {
        let error = ensure_asserted_evidence_not_deleted(&contribution(), true).unwrap_err();
        assert_eq!(
            error.to_string(),
            "contribution contribution_a is referenced by an assertion and cannot be deleted"
        );
        let error = ensure_asserted_evidence_not_deleted(&synthesis_packet(), true).unwrap_err();
        assert_eq!(
            error.to_string(),
            "synthesis packet synthesis_a is referenced by an assertion and cannot be deleted"
        );
    }

    fn contribution() -> Contribution {
        stored(ideation::contribution("contribution_a"))
    }

    fn synthesis_packet() -> SynthesisPacket {
        stored(ideation::synthesis("synthesis_a"))
    }

    /// The stored form of the request body `record` in the scope `default`.
    fn stored<T: serde::de::DeserializeOwned>(mut record: serde_json::Value) -> T {
        record["scope_id"] = json!("default");
        serde_json::from_value(record).unwrap()
    }
}
