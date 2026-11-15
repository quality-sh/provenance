//! The read of the landing batches of a scope, and the checks that the read
//! runs on each batch before it applies the batch.
//!
//! A read applies the batches in the order of the landings file. A record of
//! a later batch replaces the record of an earlier batch that has its id. The
//! checks depend on that order: a batch cannot add a second proposal,
//! assertion or disposition with an id, and it cannot change a contribution
//! or a synthesis packet that an assertion of an earlier batch cites.

use super::ideation_batches::{
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, overlay_records,
};
use super::IdeationLandingBatch;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, DuplicateImmutableId, IdeationRecord,
    ProposalCard, SynthesisPacket,
};
use std::collections::{BTreeMap, BTreeSet};

/// The ideation records of a scope: the records of its direct files, with
/// the landing batches applied in the order of the landings file.
#[derive(Debug, Default)]
pub struct IdeationRecords {
    pub contributions: Vec<Contribution>,
    pub synthesis_packets: Vec<SynthesisPacket>,
    pub proposals: Vec<ProposalCard>,
    pub assertions: Vec<AssertionRecord>,
    pub dispositions: Vec<DispositionRecord>,
    /// The ids of the proposals, the assertions and the dispositions.
    immutable_ids: [BTreeSet<String>; 3],
}

/// A check of the read that a landing batch fails.
#[derive(Debug)]
pub struct LandingRefusal {
    /// The record of the batch that fails the check, as `<list>/<id>`, where
    /// the list is the field of the batch that holds the record.
    pub record_id: String,
    pub cause: RefusalCause,
}

/// Why a check of the read refuses a landing batch.
#[derive(Debug)]
pub enum RefusalCause {
    /// The batch changes a contribution or a synthesis packet that an
    /// assertion cites.
    Evidence(anyhow::Error),
    /// The batch holds a proposal, an assertion or a disposition whose id
    /// the records or the batch already hold.
    Duplicate(DuplicateImmutableId),
}

impl std::fmt::Display for RefusalCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Evidence(error) => std::fmt::Display::fmt(error, f),
            Self::Duplicate(duplicate) => std::fmt::Display::fmt(duplicate, f),
        }
    }
}

impl IdeationRecords {
    /// The records of the direct files, with no landing batch applied.
    /// Refuses the first proposal, assertion or disposition, in that order,
    /// whose id an earlier record of its list holds.
    pub fn new(
        contributions: Vec<Contribution>,
        synthesis_packets: Vec<SynthesisPacket>,
        proposals: Vec<ProposalCard>,
        assertions: Vec<AssertionRecord>,
        dispositions: Vec<DispositionRecord>,
    ) -> Result<Self, DuplicateImmutableId> {
        let none = BTreeSet::new();
        ensure_new_ids(&proposals, &none, |duplicate| duplicate)?;
        ensure_new_ids(&assertions, &none, |duplicate| duplicate)?;
        ensure_new_ids(&dispositions, &none, |duplicate| duplicate)?;
        let immutable_ids = [ids(&proposals), ids(&assertions), ids(&dispositions)];
        Ok(Self {
            contributions,
            synthesis_packets,
            proposals,
            assertions,
            dispositions,
            immutable_ids,
        })
    }

    /// Refuses `batch` when it changes a contribution or a synthesis packet
    /// that an assertion of these records cites, or when it holds a proposal,
    /// an assertion or a disposition whose id these records or the batch
    /// already hold. The evidence check comes first, then the proposals, the
    /// assertions and the dispositions.
    pub fn check(&self, batch: &IdeationLandingBatch) -> Result<(), LandingRefusal> {
        let refused = |list: &str, id: &str, cause| LandingRefusal {
            record_id: format!("{list}/{id}"),
            cause,
        };
        let evidence = |list: &'static str, id: &str| {
            let id = id.to_owned();
            move |error| refused(list, &id, RefusalCause::Evidence(error))
        };
        for replacement in &batch.contributions {
            if let Some(existing) = find(&self.contributions, replacement) {
                let id = replacement.immutable_id();
                ensure_asserted_contribution_unchanged(existing, replacement, &self.assertions)
                    .map_err(evidence("contributions", id))?;
            }
        }
        for replacement in &batch.synthesis_packets {
            if let Some(existing) = find(&self.synthesis_packets, replacement) {
                let id = replacement.immutable_id();
                ensure_asserted_synthesis_unchanged(existing, replacement, &self.assertions)
                    .map_err(evidence("synthesis_packets", id))?;
            }
        }
        let [proposals, assertions, dispositions] = &self.immutable_ids;
        let second = |list: &'static str| {
            move |duplicate: DuplicateImmutableId| {
                let id = duplicate.id.clone();
                refused(list, &id, RefusalCause::Duplicate(duplicate))
            }
        };
        ensure_new_ids(&batch.proposals, proposals, second("proposals"))?;
        ensure_new_ids(&batch.assertions, assertions, second("assertions"))?;
        ensure_new_ids(&batch.dispositions, dispositions, second("dispositions"))
    }

    /// Applies `batch`: each record of the batch replaces the record with its
    /// id, or is added when no record has its id.
    pub fn apply(&mut self, batch: IdeationLandingBatch) {
        let [proposals, assertions, dispositions] = &mut self.immutable_ids;
        proposals.extend(ids(&batch.proposals));
        assertions.extend(ids(&batch.assertions));
        dispositions.extend(ids(&batch.dispositions));
        overlay_records(&mut self.contributions, batch.contributions);
        overlay_records(&mut self.synthesis_packets, batch.synthesis_packets);
        overlay_records(&mut self.proposals, batch.proposals);
        overlay_records(&mut self.assertions, batch.assertions);
        overlay_records(&mut self.dispositions, batch.dispositions);
    }

    /// The records of each field of a landing batch, by id, as JSON, in the
    /// order of the fields.
    pub fn lists(&self) -> serde_json::Result<[(&'static str, Records); 5]> {
        Ok([
            ("contributions", by_id(&self.contributions)?),
            ("synthesis_packets", by_id(&self.synthesis_packets)?),
            ("proposals", by_id(&self.proposals)?),
            ("assertions", by_id(&self.assertions)?),
            ("dispositions", by_id(&self.dispositions)?),
        ])
    }
}

/// Records as JSON, by id.
pub type Records = BTreeMap<String, serde_json::Value>;

fn by_id<T: IdeationRecord + serde::Serialize>(records: &[T]) -> serde_json::Result<Records> {
    records
        .iter()
        .map(|record| {
            let id = record.immutable_id().to_owned();
            Ok((id, serde_json::to_value(record)?))
        })
        .collect()
}

fn find<'a, T: IdeationRecord>(records: &'a [T], record: &T) -> Option<&'a T> {
    records
        .iter()
        .find(|current| current.immutable_id() == record.immutable_id())
}

fn ids<T: IdeationRecord>(records: &[T]) -> BTreeSet<String> {
    records
        .iter()
        .map(|record| record.immutable_id().to_owned())
        .collect()
}

/// Refuses the first of `records` whose id `held` or an earlier record of
/// `records` holds, with `at_fault` of it.
fn ensure_new_ids<T: IdeationRecord, E>(
    records: &[T],
    held: &BTreeSet<String>,
    at_fault: impl Fn(DuplicateImmutableId) -> E,
) -> Result<(), E> {
    let mut seen = BTreeSet::new();
    for record in records {
        let id = record.immutable_id();
        if held.contains(id) || !seen.insert(id) {
            return Err(at_fault(DuplicateImmutableId {
                kind: T::KIND,
                id: id.to_owned(),
            }));
        }
    }
    Ok(())
}
