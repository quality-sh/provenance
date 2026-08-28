use super::{
    read_ideation_landings, read_jsonl, read_legacy_dispositions, IdeationLandingBatch, StateStore,
};
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, IdeationAggregate, ProposalCard, ScopeId,
    SynthesisPacket,
};
use provenance_macros::rule;
use std::collections::{BTreeMap, BTreeSet};

impl StateStore {
    pub(super) fn with_lifecycle_lock<R>(
        &self,
        scope: &ScopeId,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.with_repository_publication(|| {
            crate::jsonl::with_advisory_lock(&self.layout.lifecycle_lock_path(scope), operation)
        })
    }

    pub(super) fn list_ideation_landings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<IdeationLandingBatch>> {
        read_ideation_landings(&shards::ideation_landings_path(&self.layout, scope))
    }

    pub fn land_ideation_batch(
        &self,
        scope: &ScopeId,
        incoming: IdeationLandingBatch,
        replace: bool,
    ) -> anyhow::Result<()> {
        self.with_lifecycle_lock(scope, || {
            self.write_ideation_batch(scope, incoming, replace)
        })
    }

    pub(super) fn write_ideation_batch(
        &self,
        scope: &ScopeId,
        incoming: IdeationLandingBatch,
        replace: bool,
    ) -> anyhow::Result<()> {
        ensure_scope(scope, &incoming)?;
        let path = shards::ideation_landings_path(&self.layout, scope);
        self.mutate_jsonl_records(&path, |landings: &mut Vec<IdeationLandingBatch>| {
            let mut contributions = self.list_contributions(scope)?;
            let mut synthesis_packets = self.list_synthesis_packets(scope)?;
            let mut proposals = self.list_proposal_definitions(scope)?;
            let mut assertions = self.list_assertion_records(scope)?;
            let mut dispositions = self.list_dispositions(scope)?;
            merge_immutable("proposal", &mut proposals, &incoming.proposals, |r| {
                r.id.as_str()
            })?;
            for replacement in &incoming.contributions {
                if let Some(existing) = contributions
                    .iter()
                    .find(|record| record.id == replacement.id)
                {
                    ensure_asserted_contribution_unchanged(existing, replacement, &assertions)?;
                }
            }
            for replacement in &incoming.synthesis_packets {
                if let Some(existing) = synthesis_packets
                    .iter()
                    .find(|record| record.id == replacement.id)
                {
                    ensure_asserted_synthesis_unchanged(existing, replacement, &assertions)?;
                }
            }
            merge_replaceable(
                "contribution",
                &mut contributions,
                &incoming.contributions,
                replace,
                |r| r.id.as_str(),
            )?;
            merge_replaceable(
                "synthesis packet",
                &mut synthesis_packets,
                &incoming.synthesis_packets,
                replace,
                |r| r.id.as_str(),
            )?;
            merge_immutable("assertion", &mut assertions, &incoming.assertions, |r| {
                r.id.as_str()
            })?;
            merge_immutable(
                "disposition",
                &mut dispositions,
                &incoming.dispositions,
                |r| r.id.as_str(),
            )?;
            for proposal in &incoming.proposals {
                // Read as the aggregate reads it, so the two cannot disagree
                // about a landing batch: the intrinsic rule is what a live
                // proposal row may claim, and a terminal row is legacy history
                // that `validate_ideation_aggregate` judges by its shipped
                // fingerprint instead.
                if proposal.promotion_state == provenance_core::PromotionState::Proposed {
                    provenance_core::validate_proposal_intrinsic(proposal)?;
                }
            }
            let manifest = self.manifest()?;
            provenance_core::validate_ideation_aggregate(IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                disposition_actor_ids: &manifest.disposition_actor_ids,
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
            landings.push(incoming);
            Ok(())
        })
    }

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

/// Evidence an assertion rests on cannot be edited afterwards.
///
/// Once any assertion cites a record, that record is frozen: a replacement is
/// accepted only when it serializes to exactly what is already stored. A
/// record no assertion cites may change freely.
///
/// The caller supplies the reading of "cited by an assertion" for its record
/// kind, because that reading differs by kind and the freeze does not: a
/// contribution is reached through the claim ids an assertion supports, a
/// synthesis packet through the packet id an assertion names.
#[rule("rule_asserted_evidence_immutable")]
pub fn ensure_asserted_evidence_unchanged<T: serde::Serialize>(
    kind: &str,
    id: &str,
    existing: &T,
    replacement: &T,
    cited_by_assertion: impl FnOnce() -> bool,
) -> anyhow::Result<()> {
    if serde_json::to_value(existing)? == serde_json::to_value(replacement)? {
        return Ok(());
    }
    anyhow::ensure!(
        !cited_by_assertion(),
        "{kind} {id} is referenced by an assertion and cannot be replaced"
    );
    Ok(())
}

/// The kinds the freeze names, so a caller in another crate spells them the
/// same way this one does.
pub const CONTRIBUTION_KIND: &str = "contribution";
pub const SYNTHESIS_KIND: &str = "synthesis packet";

/// An assertion cites a contribution through the claim ids it supports.
pub fn assertion_cites_contribution(
    contribution: &Contribution,
    assertions: &[AssertionRecord],
) -> bool {
    assertions.iter().any(|assertion| {
        contribution
            .material_claims
            .iter()
            .any(|claim| assertion.supporting_claim_ids.contains(&claim.claim_id))
    })
}

/// An assertion cites a synthesis packet by the packet id it names.
pub fn assertion_cites_synthesis(packet: &SynthesisPacket, assertions: &[AssertionRecord]) -> bool {
    assertions
        .iter()
        .any(|assertion| assertion.synthesis_packet_id == packet.id)
}

pub fn ensure_asserted_contribution_unchanged(
    existing: &Contribution,
    replacement: &Contribution,
    assertions: &[AssertionRecord],
) -> anyhow::Result<()> {
    ensure_asserted_evidence_unchanged(
        CONTRIBUTION_KIND,
        existing.id.as_str(),
        existing,
        replacement,
        || assertion_cites_contribution(existing, assertions),
    )
}

pub fn ensure_asserted_synthesis_unchanged(
    existing: &SynthesisPacket,
    replacement: &SynthesisPacket,
    assertions: &[AssertionRecord],
) -> anyhow::Result<()> {
    ensure_asserted_evidence_unchanged(
        SYNTHESIS_KIND,
        existing.id.as_str(),
        existing,
        replacement,
        || assertion_cites_synthesis(existing, assertions),
    )
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

fn ensure_scope(scope: &ScopeId, batch: &IdeationLandingBatch) -> anyhow::Result<()> {
    for (kind, actual) in batch
        .contributions
        .iter()
        .map(|r| ("contribution", &r.scope_id))
        .chain(
            batch
                .synthesis_packets
                .iter()
                .map(|r| ("synthesis packet", &r.scope_id)),
        )
        .chain(batch.proposals.iter().map(|r| ("proposal", &r.scope_id)))
        .chain(batch.assertions.iter().map(|r| ("assertion", &r.scope_id)))
        .chain(
            batch
                .dispositions
                .iter()
                .map(|r| ("disposition", &r.scope_id)),
        )
    {
        anyhow::ensure!(actual == scope, "{kind} scope_id must match landing scope");
    }
    Ok(())
}

fn merge_replaceable<T: Clone>(
    kind: &str,
    existing: &mut Vec<T>,
    incoming: &[T],
    replace: bool,
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = id(record);
        anyhow::ensure!(
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        if let Some(index) = existing.iter().position(|current| id(current) == record_id) {
            anyhow::ensure!(replace, "{kind} {record_id} already exists");
            existing[index] = record.clone();
        } else {
            existing.push(record.clone());
        }
    }
    Ok(())
}

fn merge_immutable<T: Clone>(
    kind: &str,
    existing: &mut Vec<T>,
    incoming: &[T],
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = id(record);
        anyhow::ensure!(
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        anyhow::ensure!(
            !existing.iter().any(|current| id(current) == record_id),
            "{kind} {record_id} already exists and is immutable"
        );
        existing.push(record.clone());
    }
    Ok(())
}

/// The retired `promotion_decisions.jsonl` shard holds history, not decisions.
///
/// The shard is admitted only when its rows are the shipped-v1 audit itself,
/// which `provenance-core` pins as a SHA-256 of the records. Membership is a
/// property of the whole set, so a row appended beside genuine history changes
/// the fingerprint and takes the whole shard down with it: a decision taken
/// now belongs to the modern lifecycle in `dispositions.jsonl`, which checks
/// the actor against the repository allowlist and demands an assertion behind
/// an acceptance. An empty shard passes, having claimed nothing.
///
/// Every row must also name a proposal that has already left `proposed`, the
/// reading that makes the shard history rather than a decision surface.
///
/// This gate reads the shard as it stands on disk. The same fingerprint is
/// checked again inside `validate_ideation_aggregate`, over the dispositions
/// of terminal proposals from every source, so a forged row that reaches the
/// aggregate by another path is caught there.
#[rule("rule_legacy_shard_frozen")]
pub(super) fn validate_legacy_disposition_shard(
    legacy_dispositions: &[DispositionRecord],
    proposals: &[ProposalCard],
) -> anyhow::Result<()> {
    if legacy_dispositions.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        provenance_core::is_shipped_legacy_disposition_audit(legacy_dispositions),
        LEGACY_SHARD_REFUSAL
    );
    for disposition in legacy_dispositions {
        anyhow::ensure!(
            proposals.iter().any(|proposal| {
                proposal.id == disposition.proposal_id
                    && proposal.promotion_state != provenance_core::PromotionState::Proposed
            }),
            LEGACY_SHARD_REFUSAL
        );
    }
    Ok(())
}

/// One refusal for both halves of the gate: the shard either is the shipped
/// audit or it is not, and a caller told which half it failed would learn only
/// how to forge the other.
const LEGACY_SHARD_REFUSAL: &str =
    "deprecated promotion_decisions.jsonl accepts only the frozen shipped-v1 disposition audit";

pub(super) fn overlay_records<T>(records: &mut Vec<T>, incoming: Vec<T>, id: impl Fn(&T) -> &str) {
    for record in incoming {
        if let Some(index) = records
            .iter()
            .position(|current| id(current) == id(&record))
        {
            records[index] = record;
        } else {
            records.push(record);
        }
    }
    records.sort_by(|a, b| id(a).cmp(id(b)));
}
