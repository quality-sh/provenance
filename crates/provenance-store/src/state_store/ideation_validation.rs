use super::landing_read::IdeationRecords;
use super::message_ids::ShardLine;
use super::shared_parses::read_jsonl_shared;
use super::StateStore;
use crate::refusal::state_fault;
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, DuplicateImmutableId, IdeationRecord,
    ProposalCard, ScopeId, SynthesisPacket,
};
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
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        self.with_repository_read(|held| {
            held.validate_ideation_scope_snapshot(scope, disposition_actor_ids)
        })
    }

    fn validate_ideation_scope_snapshot(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        let contributions: Vec<Contribution> =
            read_jsonl_shared(self, &shards::contributions_path(&self.layout, scope))?;
        let synthesis_packets: Vec<SynthesisPacket> =
            read_jsonl_shared(self, &shards::synthesis_packets_path(&self.layout, scope))?;
        let proposals: Vec<ProposalCard> =
            read_jsonl_shared(self, &shards::proposal_cards_path(&self.layout, scope))?;
        let assertions: Vec<AssertionRecord> =
            read_jsonl_shared(self, &shards::assertion_records_path(&self.layout, scope))?;
        let dispositions: Vec<DispositionRecord> =
            read_jsonl_shared(self, &shards::dispositions_path(&self.layout, scope))?;
        let stored = |duplicate| self.duplicate_id_fault(scope, duplicate);
        insert_all(&proposals, &mut BTreeSet::new(), stored)?;
        insert_all(&assertions, &mut BTreeSet::new(), stored)?;
        insert_all(&dispositions, &mut BTreeSet::new(), stored)?;
        let mut records = IdeationRecords::new(
            contributions,
            synthesis_packets,
            proposals,
            assertions,
            dispositions,
        );
        let landings = shards::ideation_landings_path(&self.layout, scope);
        for (index, batch) in self.list_ideation_landings(scope)?.into_iter().enumerate() {
            let at = ShardLine::new(&landings, index + 1);
            records
                .check(&batch, |duplicate| state_fault(&at, duplicate.to_string()))
                .map_err(|refusal| refusal.error)?;
            records.apply(batch);
        }
        self.check_ideation_aggregate(
            scope,
            provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                disposition_actor_ids,
                contributions: &records.contributions,
                synthesis_packets: &records.synthesis_packets,
                proposals: &records.proposals,
                assertions: &records.assertions,
                dispositions: &records.dispositions,
            },
        )?;
        let canonical_artifacts = self.canonical_artifact_index(scope)?;
        for disposition in &records.dispositions {
            canonical_artifacts.ensure_exists(disposition.canonical_artifact.as_ref())?;
        }
        Ok(())
    }
}

/// Adds the id of each of `records` to `seen`. The refusal of an id that
/// `seen` already holds is `at_fault` of it.
fn insert_all<T: IdeationRecord>(
    records: &[T],
    seen: &mut BTreeSet<String>,
    at_fault: impl Fn(DuplicateImmutableId) -> anyhow::Error,
) -> anyhow::Result<()> {
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
