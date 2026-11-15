use super::read_budget::ensure_slice_within_read_budget;
use super::{ensure_new_ids_assignable, IdeationLandingBatch, StateStore};
use crate::publication::ShardBytes;
use crate::shards;
use camino::Utf8PathBuf;
use provenance_core::{
    AssertionRecord, Contribution, IdeationRecord, IdeationRecordKind, ProposalCard, ScopeId,
    SynthesisPacket,
};
use provenance_macros::rule;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeSet;
use std::convert::Infallible;

impl StateStore {
    pub fn land_ideation_batch(
        &self,
        scope: &ScopeId,
        incoming: IdeationLandingBatch,
        replace: bool,
    ) -> anyhow::Result<()> {
        self.with_repository_publication(|held| held.write_ideation_batch(scope, incoming, replace))
    }

    /// Checks the records of `incoming` with the records of `scope`, and
    /// publishes them with [`Self::publish_kind_files`].
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
        self.ensure_writable()?;
        self.with_repository_publication(|held| {
            let mut contributions = held.list_contributions(scope)?;
            let mut synthesis_packets = held.list_synthesis_packets(scope)?;
            held.ensure_batch_targets_exist(scope, &contributions, &synthesis_packets, &incoming)?;
            let mut proposals = held.list_proposal_definitions(scope)?;
            let mut assertions = held.list_assertion_records(scope)?;
            let mut dispositions = held.list_dispositions(scope)?;
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
            merge_immutable(&mut proposals, &incoming.proposals)?;
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
            merge_replaceable(&mut contributions, &incoming.contributions, replace)?;
            merge_replaceable(&mut synthesis_packets, &incoming.synthesis_packets, replace)?;
            merge_immutable(&mut assertions, &incoming.assertions)?;
            merge_immutable(&mut dispositions, &incoming.dispositions)?;
            validate_batch_proposals(&incoming.proposals)?;
            let manifest = held.manifest()?;
            held.check_ideation_aggregate(
                scope,
                provenance_core::IdeationAggregate {
                    legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                    disposition_actor_ids: &manifest.disposition_actor_ids,
                    contributions: &contributions,
                    synthesis_packets: &synthesis_packets,
                    proposals: &proposals,
                    assertions: &assertions,
                    dispositions: &dispositions,
                },
            )?;
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
            let canonical_artifacts = held.canonical_artifact_index(scope)?;
            for disposition in &dispositions {
                canonical_artifacts.ensure_exists(disposition.canonical_artifact.as_ref())?;
            }
            held.publish_kind_files(scope, incoming)
        })
    }

    /// Writes each record of `incoming` to the state file of its kind, and
    /// publishes the changed files in one write.
    #[rule("rule_swarm_run_records_in_kind_files")]
    fn publish_kind_files(
        &self,
        scope: &ScopeId,
        incoming: IdeationLandingBatch,
    ) -> anyhow::Result<()> {
        let layout = &self.layout;
        let files = [
            self.kind_file(
                shards::contributions_path(layout, scope),
                incoming.contributions,
            )?,
            self.kind_file(
                shards::synthesis_packets_path(layout, scope),
                incoming.synthesis_packets,
            )?,
            self.kind_file(
                shards::proposal_cards_path(layout, scope),
                incoming.proposals,
            )?,
            self.kind_file(
                shards::assertion_records_path(layout, scope),
                incoming.assertions,
            )?,
            self.kind_file(
                shards::dispositions_path(layout, scope),
                incoming.dispositions,
            )?,
        ];
        self.publish_shards(files.into_iter().flatten().collect())
    }

    /// The new bytes of the state file at `path` with each of `records` at
    /// the place that [`place_in_file`] gives it. No bytes when `records` is
    /// empty.
    fn kind_file<T>(&self, path: Utf8PathBuf, records: Vec<T>) -> anyhow::Result<Option<ShardBytes>>
    where
        T: IdeationRecord + DeserializeOwned + Serialize,
    {
        if records.is_empty() {
            return Ok(None);
        }
        let contents = self.state_text(&path)?;
        let bytes = crate::jsonl::mutate_jsonl_bytes(self, &path, contents.as_deref(), |rows| {
            for record in records {
                place_in_file(rows, record);
            }
            Ok(true)
        })?;
        Ok(bytes.map(|bytes| (path, bytes)))
    }
}

fn validate_batch_proposals(proposals: &[ProposalCard]) -> anyhow::Result<()> {
    for proposal in proposals {
        // Read as the aggregate reads it, so the two cannot disagree
        // about the proposals of a landing: the intrinsic rule is what a live
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
pub fn ensure_asserted_evidence_unchanged<T: serde::Serialize + IdeationRecord>(
    existing: &T,
    replacement: &T,
    cited_by_assertion: impl FnOnce() -> bool,
) -> anyhow::Result<()> {
    if serde_json::to_value(existing)? == serde_json::to_value(replacement)? {
        return Ok(());
    }
    let (kind, id) = (T::KIND, existing.immutable_id());
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
    let cited = || assertion_cites_contribution(existing, assertions);
    ensure_asserted_evidence_unchanged(existing, replacement, cited)
}

pub fn ensure_asserted_synthesis_unchanged(
    existing: &SynthesisPacket,
    replacement: &SynthesisPacket,
    assertions: &[AssertionRecord],
) -> anyhow::Result<()> {
    let cited = || assertion_cites_synthesis(existing, assertions);
    ensure_asserted_evidence_unchanged(existing, replacement, cited)
}

fn ensure_scope(scope: &ScopeId, batch: &IdeationLandingBatch) -> anyhow::Result<()> {
    for (kind, actual) in scope_ids(&batch.contributions, |r| &r.scope_id)
        .chain(scope_ids(&batch.synthesis_packets, |r| &r.scope_id))
        .chain(scope_ids(&batch.proposals, |r| &r.scope_id))
        .chain(scope_ids(&batch.assertions, |r| &r.scope_id))
        .chain(scope_ids(&batch.dispositions, |r| &r.scope_id))
    {
        crate::write_error::ensure!(
            ScopeMismatch,
            actual == scope,
            "{kind} scope_id must match landing scope"
        );
    }
    Ok(())
}

/// The kind and the scope of each of `records`.
fn scope_ids<T: IdeationRecord>(
    records: &[T],
    scope_id: impl Fn(&T) -> &ScopeId,
) -> impl Iterator<Item = (IdeationRecordKind, &ScopeId)> {
    records
        .iter()
        .map(move |record| (T::KIND, scope_id(record)))
}

fn merge_replaceable<T: Clone + IdeationRecord>(
    existing: &mut Vec<T>,
    incoming: &[T],
    replace: bool,
) -> anyhow::Result<()> {
    let kind = T::KIND;
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = record.immutable_id();
        crate::write_error::ensure!(
            InvalidUpdate,
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        replace_checked_or_append(existing, record.clone(), |_| {
            crate::write_error::ensure!(
                AlreadyExists,
                replace,
                "{kind} {record_id} already exists"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn merge_immutable<T: Clone + IdeationRecord>(
    existing: &mut Vec<T>,
    incoming: &[T],
) -> anyhow::Result<()> {
    let kind = T::KIND;
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = record.immutable_id();
        crate::write_error::ensure!(
            InvalidUpdate,
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        crate::write_error::ensure!(
            AlreadyExists,
            !existing
                .iter()
                .any(|current| current.immutable_id() == record_id),
            "{kind} {record_id} already exists and is immutable"
        );
        existing.push(record.clone());
    }
    Ok(())
}

/// Puts `record` in the rows of the state file of its kind: in place of the
/// row with its id, or else among the rows in id order. The writers of a kind
/// whose record a write can replace (a contribution, a synthesis packet) and
/// each landing give a record its place with this function. It puts the
/// complete file in id order; `import` writes a file in the order of the
/// export file, and the next write of a record of that kind sorts it.
pub(super) fn place_in_file<T: IdeationRecord>(rows: &mut Vec<T>, record: T) {
    let Ok(()) = place_checked_in_file(rows, record, |_| Ok::<(), Infallible>(()));
}

/// [`place_in_file`], after `check_stored` reads the stored row with the id of
/// `record`. When it refuses, `rows` stays as it is.
pub(super) fn place_checked_in_file<T: IdeationRecord, E>(
    rows: &mut Vec<T>,
    record: T,
    check_stored: impl FnOnce(&T) -> Result<(), E>,
) -> Result<(), E> {
    replace_checked_or_append(rows, record, check_stored)?;
    sort_by_id(rows);
    Ok(())
}

/// Puts the new `record` among the rows of the state file of its kind, in id
/// order, for the writers of a kind whose record no write replaces. When a row
/// has the id of `record`, `rows` stays as it is and the write fails with the
/// error that `stored` gives for that row.
pub(super) fn add_to_file<T: IdeationRecord>(
    rows: &mut Vec<T>,
    record: T,
    stored: impl FnOnce(&T) -> anyhow::Error,
) -> anyhow::Result<()> {
    if let Some(row) = rows
        .iter()
        .find(|row| row.immutable_id() == record.immutable_id())
    {
        return Err(stored(row));
    }
    rows.push(record);
    sort_by_id(rows);
    Ok(())
}

/// Puts the rows of a state file in id order: the order that each landing and
/// each write of one record of the five ideation kinds gives the file.
fn sort_by_id<T: IdeationRecord>(rows: &mut [T]) {
    rows.sort_by(|a, b| a.immutable_id().cmp(b.immutable_id()));
}

/// Puts `record` in place of the record of `records` with its id, or at the
/// end when no record has its id.
pub(super) fn replace_or_append<T: IdeationRecord>(records: &mut Vec<T>, record: T) {
    let Ok(()) = replace_checked_or_append(records, record, |_| Ok::<(), Infallible>(()));
}

/// Puts `record` in place of the record of `records` with its id, or at the
/// end when no record has its id. Before a replacement, `check_stored` reads
/// the stored record with that id; when it refuses, `records` stays as it is.
pub(super) fn replace_checked_or_append<T: IdeationRecord, E>(
    records: &mut Vec<T>,
    record: T,
    check_stored: impl FnOnce(&T) -> Result<(), E>,
) -> Result<(), E> {
    match records
        .iter()
        .position(|current| current.immutable_id() == record.immutable_id())
    {
        Some(index) => {
            check_stored(&records[index])?;
            records[index] = record;
        }
        None => records.push(record),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assertion(id: &str) -> AssertionRecord {
        serde_json::from_value(serde_json::json!({"scope_id": "default", "id": id,
            "proposal_id": format!("proposal_{id}"), "synthesis_packet_id": "packet_a",
            "supporting_claim_ids": []}))
        .unwrap()
    }

    fn ids(rows: &[AssertionRecord]) -> Vec<&str> {
        rows.iter().map(|row| row.immutable_id()).collect()
    }

    /// Implementation aid: `add_to_file` puts a new record in id order and
    /// never replaces a row. No Rule binds it: no public surface reaches the
    /// refusal, because each writer refuses a stored id before it.
    #[test]
    fn add_to_file_puts_a_new_record_in_id_order_and_refuses_a_stored_id() {
        let mut rows = vec![assertion("c"), assertion("a")];
        add_to_file(&mut rows, assertion("b"), |_| anyhow::anyhow!("stored")).unwrap();
        assert_eq!(ids(&rows), ["a", "b", "c"]);
        let mut changed = assertion("b");
        changed.proposal_id = provenance_core::StableId::new("proposal_x").unwrap();
        let error = add_to_file(&mut rows, changed, |row| {
            anyhow::anyhow!("{}", row.proposal_id.as_str())
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "proposal_b");
        assert_eq!(rows, [assertion("a"), assertion("b"), assertion("c")]);
    }
}
