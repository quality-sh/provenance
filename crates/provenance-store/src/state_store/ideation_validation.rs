use super::ideation_batches::{
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, overlay_records,
    validate_legacy_disposition_shard,
};
use super::{read_jsonl, read_legacy_dispositions, IdeationLandingBatch, StateStore};
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, IdeationAggregate, ProposalCard, ScopeId,
    SynthesisPacket,
};
use std::collections::BTreeMap;

impl StateStore {
    pub fn validate_ideation_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_read(|| {
            let manifest = self.manifest()?;
            self.validate_ideation_scope_snapshot(scope, &manifest.disposition_actor_ids)
        })
    }

    pub fn validate_ideation_scope_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        self.with_repository_read(|| {
            self.validate_ideation_scope_snapshot(scope, disposition_actor_ids)
        })
    }

    fn validate_ideation_scope_snapshot(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        let mut contributions: Vec<Contribution> =
            read_jsonl(self, &shards::contributions_path(&self.layout, scope))?;
        let mut synthesis_packets: Vec<SynthesisPacket> =
            read_jsonl(self, &shards::synthesis_packets_path(&self.layout, scope))?;
        let direct_proposals: Vec<ProposalCard> =
            read_jsonl(self, &shards::proposal_cards_path(&self.layout, scope))?;
        let direct_assertions: Vec<AssertionRecord> =
            read_jsonl(self, &shards::assertion_records_path(&self.layout, scope))?;
        let mut assertions_in_order = direct_assertions.clone();
        let mut direct_dispositions: Vec<DispositionRecord> =
            read_jsonl(self, &shards::dispositions_path(&self.layout, scope))?;
        let legacy_dispositions = read_legacy_dispositions(
            self,
            &shards::legacy_promotion_decisions_path(&self.layout, scope),
        )?;
        direct_dispositions.extend(legacy_dispositions.iter().cloned());
        let mut proposals = BTreeMap::new();
        let mut assertions = BTreeMap::new();
        let mut dispositions = BTreeMap::new();
        insert_all(
            "proposal",
            &direct_proposals,
            |r| r.id.as_str(),
            &mut proposals,
        )?;
        insert_all(
            "assertion",
            &direct_assertions,
            |r| r.id.as_str(),
            &mut assertions,
        )?;
        insert_all(
            "disposition",
            &direct_dispositions,
            |r| r.id.as_str(),
            &mut dispositions,
        )?;
        for batch in self.list_ideation_landings(scope)? {
            ensure_batch_evidence_unchanged(
                &contributions,
                &synthesis_packets,
                &batch,
                &assertions_in_order,
            )?;
            insert_all(
                "proposal",
                &batch.proposals,
                |r| r.id.as_str(),
                &mut proposals,
            )?;
            insert_all(
                "assertion",
                &batch.assertions,
                |r| r.id.as_str(),
                &mut assertions,
            )?;
            insert_all(
                "disposition",
                &batch.dispositions,
                |r| r.id.as_str(),
                &mut dispositions,
            )?;
            overlay_records(&mut contributions, batch.contributions, |record| {
                record.id.as_str()
            });
            overlay_records(&mut synthesis_packets, batch.synthesis_packets, |record| {
                record.id.as_str()
            });
            assertions_in_order.extend(batch.assertions);
        }
        let proposals = self.list_proposal_definitions(scope)?;
        let assertions = self.list_assertion_records(scope)?;
        let dispositions = self.list_dispositions(scope)?;
        validate_legacy_disposition_shard(&legacy_dispositions, &proposals)?;
        provenance_core::validate_ideation_aggregate(IdeationAggregate {
            legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
            disposition_actor_ids,
            contributions: &contributions,
            synthesis_packets: &synthesis_packets,
            proposals: &proposals,
            assertions: &assertions,
            dispositions: &dispositions,
        })?;
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

fn insert_all<'a, T: serde::Serialize>(
    kind: &str,
    records: &'a [T],
    id: impl Fn(&'a T) -> &'a str,
    seen: &mut BTreeMap<String, serde_json::Value>,
) -> anyhow::Result<()> {
    for record in records {
        let record_id = id(record);
        let value = serde_json::to_value(record)?;
        anyhow::ensure!(
            seen.insert(record_id.to_owned(), value).is_none(),
            "duplicate immutable {kind} id {record_id}"
        );
    }
    Ok(())
}
