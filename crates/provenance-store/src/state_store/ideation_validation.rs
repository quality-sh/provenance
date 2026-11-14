use super::ideation_batches::{
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, overlay_records,
};
use super::message_ids::ShardLine;
use super::shared_parses::read_jsonl_shared;
use super::{IdeationLandingBatch, StateStore};
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
        let mut contributions: Vec<Contribution> =
            read_jsonl_shared(self, &shards::contributions_path(&self.layout, scope))?;
        let mut synthesis_packets: Vec<SynthesisPacket> =
            read_jsonl_shared(self, &shards::synthesis_packets_path(&self.layout, scope))?;
        let direct_proposals: Vec<ProposalCard> =
            read_jsonl_shared(self, &shards::proposal_cards_path(&self.layout, scope))?;
        let direct_assertions: Vec<AssertionRecord> =
            read_jsonl_shared(self, &shards::assertion_records_path(&self.layout, scope))?;
        let mut assertions_in_order = direct_assertions.clone();
        let direct_dispositions: Vec<DispositionRecord> =
            read_jsonl_shared(self, &shards::dispositions_path(&self.layout, scope))?;
        let mut proposals = BTreeSet::new();
        let mut assertions = BTreeSet::new();
        let mut dispositions = BTreeSet::new();
        let stored = |duplicate| self.duplicate_id_fault(scope, duplicate);
        insert_all(&direct_proposals, &mut proposals, stored)?;
        insert_all(&direct_assertions, &mut assertions, stored)?;
        insert_all(&direct_dispositions, &mut dispositions, stored)?;
        let landings = shards::ideation_landings_path(&self.layout, scope);
        for (index, batch) in self.list_ideation_landings(scope)?.into_iter().enumerate() {
            let at = ShardLine::new(&landings, index + 1);
            ensure_batch_evidence_unchanged(
                &contributions,
                &synthesis_packets,
                &batch,
                &assertions_in_order,
            )?;
            let landed = |duplicate: DuplicateImmutableId| state_fault(&at, duplicate.to_string());
            insert_all(&batch.proposals, &mut proposals, landed)?;
            insert_all(&batch.assertions, &mut assertions, landed)?;
            insert_all(&batch.dispositions, &mut dispositions, landed)?;
            overlay_records(&mut contributions, batch.contributions);
            overlay_records(&mut synthesis_packets, batch.synthesis_packets);
            assertions_in_order.extend(batch.assertions);
        }
        let proposals = self.list_proposal_definitions(scope)?;
        let assertions = self.list_assertion_records(scope)?;
        let dispositions = self.list_dispositions(scope)?;
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
        for disposition in &dispositions {
            canonical_artifacts.ensure_exists(disposition.canonical_artifact.as_ref())?;
        }
        Ok(())
    }
}

fn ensure_batch_evidence_unchanged(
    contributions: &[Contribution],
    synthesis_packets: &[SynthesisPacket],
    batch: &IdeationLandingBatch,
    assertions: &[AssertionRecord],
) -> anyhow::Result<()> {
    for replacement in &batch.contributions {
        if let Some(existing) = contributions
            .iter()
            .find(|record| record.id == replacement.id)
        {
            ensure_asserted_contribution_unchanged(existing, replacement, assertions)?;
        }
    }
    for replacement in &batch.synthesis_packets {
        if let Some(existing) = synthesis_packets
            .iter()
            .find(|record| record.id == replacement.id)
        {
            ensure_asserted_synthesis_unchanged(existing, replacement, assertions)?;
        }
    }
    Ok(())
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
