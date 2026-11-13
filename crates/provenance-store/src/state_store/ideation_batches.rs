use super::read_budget::ensure_slice_within_read_budget;
use super::{ensure_new_ids_assignable, read_ideation_landings, IdeationLandingBatch, StateStore};
use crate::families::RecordFamily;
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, IdeationRecordKind as Kind, ProposalCard, ScopeId,
    SynthesisPacket,
};
use provenance_macros::rule;
use std::collections::BTreeSet;

impl StateStore {
    /// Runs `operation` in the section of this store under the lifecycle
    /// lock of `scope`. A staged section takes no lifecycle lock: the stage
    /// lock already runs its sections one at a time, and no other process
    /// sees the stage before it publishes.
    pub(super) fn with_lifecycle_lock<R>(
        &self,
        scope: &ScopeId,
        operation: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.with_repository_publication(|held| {
            if held.stage().is_some() {
                return operation(held);
            }
            #[cfg(feature = "host")]
            return crate::jsonl::with_advisory_lock(
                &held.layout.lifecycle_lock_path(scope),
                || operation(held),
            );
            // With no lock files, the section alone runs the writes one at a time.
            #[cfg(not(feature = "host"))]
            {
                let _ = scope;
                operation(held)
            }
        })
    }

    pub(super) fn list_ideation_landings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<IdeationLandingBatch>> {
        let path = shards::ideation_landings_path(&self.layout, scope);
        super::shared_parses::shared(self, &path, || read_ideation_landings(self, &path))
    }

    pub fn land_ideation_batch(
        &self,
        scope: &ScopeId,
        incoming: IdeationLandingBatch,
        replace: bool,
    ) -> anyhow::Result<()> {
        self.with_lifecycle_lock(scope, |held| {
            held.write_ideation_batch(scope, incoming, replace)
        })
    }

    pub(super) fn write_ideation_batch(
        &self,
        scope: &ScopeId,
        incoming: IdeationLandingBatch,
        replace: bool,
    ) -> anyhow::Result<()> {
        ensure_scope(scope, &incoming)?;
        ensure_slice_within_read_budget(&incoming.contributions)?;
        ensure_slice_within_read_budget(&incoming.synthesis_packets)?;
        ensure_slice_within_read_budget(&incoming.proposals)?;
        ensure_slice_within_read_budget(&incoming.assertions)?;
        ensure_slice_within_read_budget(&incoming.dispositions)?;
        let path = shards::ideation_landings_path(&self.layout, scope);
        self.mutate_jsonl_records(&path, |landings: &mut Vec<IdeationLandingBatch>| {
            let mut contributions = self.list_contributions(scope)?;
            let mut synthesis_packets = self.list_synthesis_packets(scope)?;
            self.ensure_batch_targets_exist(scope, &contributions, &synthesis_packets, &incoming)?;
            let mut proposals = self.list_proposal_definitions(scope)?;
            let mut assertions = self.list_assertion_records(scope)?;
            let mut dispositions = self.list_dispositions(scope)?;
            ensure_new_ids_assignable(&contributions, &incoming.contributions, |record| {
                record.id.as_str()
            })?;
            ensure_new_ids_assignable(&synthesis_packets, &incoming.synthesis_packets, |record| {
                record.id.as_str()
            })?;
            ensure_new_ids_assignable(&proposals, &incoming.proposals, |record| {
                record.id.as_str()
            })?;
            ensure_new_ids_assignable(&assertions, &incoming.assertions, |record| {
                record.id.as_str()
            })?;
            ensure_new_ids_assignable(&dispositions, &incoming.dispositions, |record| {
                record.id.as_str()
            })?;
            merge_immutable(Kind::Proposal, &mut proposals, &incoming.proposals)?;
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
                Kind::Contribution,
                &mut contributions,
                &incoming.contributions,
                replace,
            )?;
            merge_replaceable(
                Kind::SynthesisPacket,
                &mut synthesis_packets,
                &incoming.synthesis_packets,
                replace,
            )?;
            merge_immutable(Kind::Assertion, &mut assertions, &incoming.assertions)?;
            merge_immutable(Kind::Disposition, &mut dispositions, &incoming.dispositions)?;
            validate_batch_proposals(&incoming.proposals)?;
            let manifest = self.manifest()?;
            self.check_ideation_aggregate(scope, provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                disposition_actor_ids: &manifest.disposition_actor_ids,
                contributions: &contributions,
                synthesis_packets: &synthesis_packets,
                proposals: &proposals,
                assertions: &assertions,
                dispositions: &dispositions,
            })?;
            for disposition in &incoming.dispositions {
                crate::write_error::ensure!(
                    InvalidUpdate,
                    !proposals.iter().any(|proposal| {
                        proposal.id == disposition.proposal_id
                            && proposal.proposal_type
                                == provenance_core::ProposalType::RecordRevision
                    }),
                    "decisions on review submissions go through the review seam"
                );
            }
            let canonical_artifacts = self.canonical_artifact_index(scope)?;
            for disposition in &dispositions {
                canonical_artifacts.ensure_exists(disposition.canonical_artifact.as_ref())?;
            }
            landings.push(incoming);
            Ok(())
        })
    }
}

fn validate_batch_proposals(proposals: &[ProposalCard]) -> anyhow::Result<()> {
    for proposal in proposals {
        // Read as the aggregate reads it, so the two cannot disagree
        // about a landing batch: the intrinsic rule is what a live
        // proposal row may claim, and a terminal row is legacy history
        // that `validate_ideation_aggregate` judges by its shipped
        // fingerprint instead.
        if proposal.promotion_state == provenance_core::PromotionState::Proposed {
            provenance_core::validate_proposal_intrinsic(proposal)?;
        }
        // A truthful binding to an exact review revision can only be
        // written by the review seam, which checks the revision under
        // the publication lock.
        crate::write_error::ensure!(
            InvalidUpdate,
            proposal.proposal_type != provenance_core::ProposalType::RecordRevision,
            "batch writes cannot create review submissions; they go through the review seam"
        );
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
    kind: Kind,
    id: &str,
    existing: &T,
    replacement: &T,
    cited_by_assertion: impl FnOnce() -> bool,
) -> anyhow::Result<()> {
    if serde_json::to_value(existing)? == serde_json::to_value(replacement)? {
        return Ok(());
    }
    crate::write_error::ensure!(
        InvalidUpdate,
        !cited_by_assertion(),
        "{kind} {id} is referenced by an assertion and cannot be replaced"
    );
    Ok(())
}

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
        Kind::Contribution,
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
        Kind::SynthesisPacket,
        existing.id.as_str(),
        existing,
        replacement,
        || assertion_cites_synthesis(existing, assertions),
    )
}

fn ensure_scope(scope: &ScopeId, batch: &IdeationLandingBatch) -> anyhow::Result<()> {
    for (kind, actual) in batch
        .contributions
        .iter()
        .map(|r| (Kind::Contribution, &r.scope_id))
        .chain(
            batch
                .synthesis_packets
                .iter()
                .map(|r| (Kind::SynthesisPacket, &r.scope_id)),
        )
        .chain(
            batch
                .proposals
                .iter()
                .map(|r| (Kind::Proposal, &r.scope_id)),
        )
        .chain(
            batch
                .assertions
                .iter()
                .map(|r| (Kind::Assertion, &r.scope_id)),
        )
        .chain(
            batch
                .dispositions
                .iter()
                .map(|r| (Kind::Disposition, &r.scope_id)),
        )
    {
        crate::write_error::ensure!(
            ScopeMismatch,
            actual == scope,
            "{kind} scope_id must match landing scope"
        );
    }
    Ok(())
}

fn merge_replaceable<T: Clone + RecordFamily>(
    kind: Kind,
    existing: &mut Vec<T>,
    incoming: &[T],
    replace: bool,
) -> anyhow::Result<()> {
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = record.record_id();
        crate::write_error::ensure!(
            InvalidUpdate,
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        if let Some(index) = existing
            .iter()
            .position(|current| current.record_id() == record_id)
        {
            crate::write_error::ensure!(
                AlreadyExists,
                replace,
                "{kind} {record_id} already exists"
            );
            existing[index] = record.clone();
        } else {
            existing.push(record.clone());
        }
    }
    Ok(())
}

fn merge_immutable<T: Clone + RecordFamily>(
    kind: Kind,
    existing: &mut Vec<T>,
    incoming: &[T],
) -> anyhow::Result<()> {
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = record.record_id();
        crate::write_error::ensure!(
            InvalidUpdate,
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        crate::write_error::ensure!(
            AlreadyExists,
            !existing.iter().any(|current| current.record_id() == record_id),
            "{kind} {record_id} already exists and is immutable"
        );
        existing.push(record.clone());
    }
    Ok(())
}

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
