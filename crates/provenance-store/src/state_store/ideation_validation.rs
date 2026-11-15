use super::landing_read::{IdeationRecords, RefusalCause};
use super::message_ids::ShardLine;
use super::shared_parses::read_jsonl_shared;
use super::StateStore;
use crate::refusal::state_fault;
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, ProposalCard, ScopeId, SynthesisPacket,
};

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
        let direct_proposals: Vec<ProposalCard> =
            read_jsonl_shared(self, &shards::proposal_cards_path(&self.layout, scope))?;
        let direct_assertions: Vec<AssertionRecord> =
            read_jsonl_shared(self, &shards::assertion_records_path(&self.layout, scope))?;
        let direct_dispositions: Vec<DispositionRecord> =
            read_jsonl_shared(self, &shards::dispositions_path(&self.layout, scope))?;
        let mut records = IdeationRecords::new(
            contributions,
            synthesis_packets,
            direct_proposals,
            direct_assertions,
            direct_dispositions,
        )
        .map_err(|duplicate| self.duplicate_id_fault(scope, duplicate))?;
        let landings = shards::ideation_landings_path(&self.layout, scope);
        for (index, batch) in self.list_ideation_landings(scope)?.into_iter().enumerate() {
            let at = ShardLine::new(&landings, index + 1);
            records
                .check(&batch)
                .map_err(|refusal| match refusal.cause {
                    RefusalCause::Evidence(error) => error,
                    RefusalCause::Duplicate(duplicate) => state_fault(&at, duplicate.to_string()),
                })?;
            records.apply(batch);
        }
        let proposals = self.list_proposal_definitions(scope)?;
        let assertions = self.list_assertion_records(scope)?;
        let dispositions = self.list_dispositions(scope)?;
        self.check_ideation_aggregate(
            scope,
            provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                disposition_actor_ids,
                contributions: &records.contributions,
                synthesis_packets: &records.synthesis_packets,
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
