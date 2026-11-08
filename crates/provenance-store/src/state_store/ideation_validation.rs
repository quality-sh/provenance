use super::ideation_batches::{
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, overlay_records,
};
use super::{read_jsonl, IdeationLandingBatch, StateStore};
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, IdeationAggregate, ProposalCard, ScopeId,
    SynthesisPacket,
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
            read_jsonl(self, &shards::contributions_path(&self.layout, scope))?;
        let mut synthesis_packets: Vec<SynthesisPacket> =
            read_jsonl(self, &shards::synthesis_packets_path(&self.layout, scope))?;
        let direct_proposals: Vec<ProposalCard> =
            read_jsonl(self, &shards::proposal_cards_path(&self.layout, scope))?;
        let direct_assertions: Vec<AssertionRecord> =
            read_jsonl(self, &shards::assertion_records_path(&self.layout, scope))?;
        let mut assertions_in_order = direct_assertions.clone();
        let direct_dispositions: Vec<DispositionRecord> =
            read_jsonl(self, &shards::dispositions_path(&self.layout, scope))?;
        let mut proposals = BTreeSet::new();
        let mut assertions = BTreeSet::new();
        let mut dispositions = BTreeSet::new();
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
        // The proposal, assertion and disposition lists that the list
        // readers give: the direct records with each landing laid over them.
        let mut proposal_definitions = direct_proposals.clone();
        let mut assertion_records = direct_assertions.clone();
        let mut disposition_records = direct_dispositions.clone();
        for batch in self.list_ideation_landings(scope)? {
            overlay_records(
                &mut proposal_definitions,
                batch.proposals.clone(),
                |record| record.id.as_str(),
            );
            overlay_records(&mut assertion_records, batch.assertions.clone(), |record| {
                record.id.as_str()
            });
            overlay_records(
                &mut disposition_records,
                batch.dispositions.clone(),
                |record| record.id.as_str(),
            );
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
        let proposals = proposal_definitions;
        let assertions = assertion_records;
        let dispositions = disposition_records;
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

fn insert_all<'a, T>(
    kind: &str,
    records: &'a [T],
    id: impl Fn(&'a T) -> &'a str,
    seen: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for record in records {
        let record_id = id(record);
        anyhow::ensure!(
            seen.insert(record_id.to_owned()),
            "duplicate immutable {kind} id {record_id}"
        );
    }
    Ok(())
}
