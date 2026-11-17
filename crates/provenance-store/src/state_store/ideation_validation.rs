use super::StateStore;
use provenance_core::{DuplicateImmutableId, IdeationRecord, ScopeId};
use std::collections::BTreeSet;

impl StateStore {
    pub fn validate_ideation_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_read(|held| {
            let manifest = held.manifest()?;
            held.validate_ideation_scope_snapshot(scope, &manifest.disposition_actor_ids)
        })
    }

    pub fn validate_ideation_scope_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &provenance_core::Reviewers,
    ) -> anyhow::Result<()> {
        self.with_repository_read(|held| {
            held.validate_ideation_scope_snapshot(scope, disposition_actor_ids)
        })
    }

    fn validate_ideation_scope_snapshot(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &provenance_core::Reviewers,
    ) -> anyhow::Result<()> {
        let contributions = self.list_contributions(scope)?;
        let synthesis_packets = self.list_synthesis_packets(scope)?;
        let proposals = self.list_proposal_definitions(scope)?;
        let assertions = self.list_assertion_records(scope)?;
        let dispositions = self.list_dispositions(scope)?;
        let stored = |duplicate| self.duplicate_id_fault(scope, duplicate);
        ensure_unique_ids(&proposals, stored)?;
        ensure_unique_ids(&assertions, stored)?;
        ensure_unique_ids(&dispositions, stored)?;
        self.check_ideation_aggregate(
            scope,
            provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                disposition_actor_ids,
                contributions: &contributions,
                synthesis_packets: &synthesis_packets,
                proposals: &proposals,
                assertions: &assertions,
                dispositions: &dispositions,
            },
        )?;
        let canonical_artifacts = self.canonical_artifact_index(scope)?;
        self.ensure_dispositions_name_records(scope, &dispositions, |kind, id| {
            canonical_artifacts.contains(kind, id)
        })
    }
}

/// Refuses the second record of `records` with an id, with `at_fault` of
/// that id.
fn ensure_unique_ids<T: IdeationRecord>(
    records: &[T],
    at_fault: impl Fn(DuplicateImmutableId) -> anyhow::Error,
) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for record in records {
        let id = record.immutable_id();
        if !seen.insert(id.to_owned()) {
            return Err(at_fault(DuplicateImmutableId {
                kind: T::KIND,
                id: id.to_owned(),
            }));
        }
    }
    Ok(())
}
