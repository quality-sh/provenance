//! Ideation scope validation.
//!
//! Read-only checks over one scope's ideation shards and the manifest's
//! disposition actor ids. The locked and guarded entries share the
//! snapshot body, so a caller inside a held publication guard never
//! requests the lock again.

use super::ideation_batches::{
    ensure_batch_evidence_unchanged, insert_all, overlay_records, validate_legacy_disposition_shard,
};
use super::readers::read_legacy_dispositions;
use super::{
    read_jsonl, AssertionRecord, Contribution, DispositionRecord, ProposalCard, ScopeId,
    StateStore, SynthesisPacket,
};
use crate::shards;
use provenance_core::IdeationAggregate;
use std::collections::BTreeMap;

impl StateStore {
    pub fn validate_ideation_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|| {
            let manifest = self.manifest()?;
            self.validate_ideation_scope_snapshot(scope, &manifest.disposition_actor_ids)
        })
    }

    /// Validates under a publication guard the caller holds, without
    /// requesting the lock again.
    pub fn validate_ideation_scope_under_guard(
        &self,
        guard: &crate::publication::guard::PublicationGuard,
        scope: &ScopeId,
    ) -> anyhow::Result<()> {
        let manifest = self.manifest_under_guard(guard)?;
        self.validate_ideation_scope_snapshot(scope, &manifest.disposition_actor_ids)
    }

    pub fn validate_ideation_scope_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        self.validate_ideation_scope_snapshot(scope, disposition_actor_ids)
    }

    pub(crate) fn validate_ideation_scope_snapshot(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        let mut contributions: Vec<Contribution> =
            read_jsonl(&shards::contributions_path(&self.layout, scope))?;
        let mut synthesis_packets: Vec<SynthesisPacket> =
            read_jsonl(&shards::synthesis_packets_path(&self.layout, scope))?;
        let direct_proposals: Vec<ProposalCard> =
            read_jsonl(&shards::proposal_cards_path(&self.layout, scope))?;
        let direct_assertions: Vec<AssertionRecord> =
            read_jsonl(&shards::assertion_records_path(&self.layout, scope))?;
        let mut assertions_in_order = direct_assertions.clone();
        let mut direct_dispositions: Vec<DispositionRecord> =
            read_jsonl(&shards::dispositions_path(&self.layout, scope))?;
        let legacy_dispositions = read_legacy_dispositions(
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
