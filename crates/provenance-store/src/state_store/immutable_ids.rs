//! The ideation records whose ids are immutable: proposals, assertions and
//! dispositions.
//!
//! A hand edit or a branch merge can put one id on two lines of the file of
//! one of these families. The list readers keep the line of each row and do
//! not check the ids. The ideation scope check and each write that runs the
//! aggregate check of the ideation records refuse that state with
//! `invalid_state_file` at the second line. The message names the two lines.

use super::ideation_batches::overlay_records;
use super::message_ids::ShardLine;
use super::readers::read_jsonl_located;
use super::shared_parses::shared;
use super::{IdeationLandingBatch, ProvenanceLayout, StateStore};
use crate::shards;
use camino::Utf8PathBuf;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, DuplicateImmutableId, IdeationAggregate,
    ProposalCard, ScopeId, SynthesisPacket,
};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

/// A family of ideation records whose ids are immutable.
pub(super) trait ImmutableRecord: DeserializeOwned + Clone + Send + 'static {
    /// The name of the family in a refusal.
    const KIND: &'static str;
    /// The file of the family in `scope`.
    fn path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf;
    /// The records of the family in a landing batch.
    fn landed(batch: IdeationLandingBatch) -> Vec<Self>;
    fn id(&self) -> &str;
}

macro_rules! immutable_record {
    ($record:ty, $kind:literal, $path:ident, $field:ident) => {
        impl ImmutableRecord for $record {
            const KIND: &'static str = $kind;
            fn path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                shards::$path(layout, scope)
            }
            fn landed(batch: IdeationLandingBatch) -> Vec<Self> {
                batch.$field
            }
            fn id(&self) -> &str {
                self.id.as_str()
            }
        }
    };
}

immutable_record!(ProposalCard, "proposal", proposal_cards_path, proposals);
immutable_record!(AssertionRecord, "assertion", assertion_records_path, assertions);
immutable_record!(DispositionRecord, "disposition", dispositions_path, dispositions);

/// The id and the line of each row of the file of one family.
pub(super) struct IdLines {
    kind: &'static str,
    path: Utf8PathBuf,
    ids: Vec<(String, usize)>,
}

impl IdLines {
    /// The located check: `invalid_state_file` at the second line of the
    /// first id that `select` accepts and that the file holds on two lines.
    /// The message names the first line and the second line of the id.
    fn check(&self, select: impl Fn(&str) -> bool) -> anyhow::Result<()> {
        let mut first_lines = BTreeMap::new();
        for (id, line) in self.ids.iter().filter(|(id, _)| select(id)) {
            if let Some(first) = first_lines.insert(id.as_str(), *line) {
                return Err(crate::refusal::state_fault(
                    &ShardLine::new(&self.path, *line),
                    format!(
                        "duplicate immutable {} id {id} at line {first} and at line {line}",
                        self.kind
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// The rows of the file of one family, each with its line.
pub(super) struct ImmutableRows<T> {
    path: Utf8PathBuf,
    rows: Vec<(T, usize)>,
}

impl<T: ImmutableRecord> ImmutableRows<T> {
    /// The records, or the refusal of the located check for the first id on
    /// two lines.
    pub(super) fn unique(self) -> anyhow::Result<Vec<T>> {
        let (records, lines) = self.split();
        lines.check(|_| true)?;
        Ok(records)
    }

    fn split(self) -> (Vec<T>, IdLines) {
        let ids = self
            .rows
            .iter()
            .map(|(record, line)| (record.id().to_owned(), *line))
            .collect();
        let records = self.rows.into_iter().map(|(record, _)| record).collect();
        let lines = IdLines {
            kind: T::KIND,
            path: self.path,
            ids,
        };
        (records, lines)
    }
}

/// The records of one family as its list reader gives them: the rows of its
/// file with the records of the landing batches over them. It keeps the line
/// of each row of the file and does not check the ids.
pub(super) struct Listed<T> {
    records: Vec<T>,
    lines: IdLines,
}

impl<T> Listed<T> {
    pub(super) fn into_records(self) -> Vec<T> {
        self.records
    }
}

impl<T> Deref for Listed<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Vec<T> {
        &self.records
    }
}

impl<T> DerefMut for Listed<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.records
    }
}

impl<'a, T> IntoIterator for &'a Listed<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.iter()
    }
}

/// The ideation records of one scope for the aggregate check of a write or
/// of the scope check.
pub(super) struct IdeationRecords<'a> {
    pub(super) disposition_actor_ids: &'a [String],
    pub(super) contributions: &'a [Contribution],
    pub(super) synthesis_packets: &'a [SynthesisPacket],
    pub(super) proposals: &'a Listed<ProposalCard>,
    pub(super) assertions: &'a Listed<AssertionRecord>,
    pub(super) dispositions: &'a Listed<DispositionRecord>,
}

impl IdeationRecords<'_> {
    /// [`provenance_core::validate_ideation_aggregate`] over these records,
    /// with the shipped legacy policy. When the aggregate check finds a
    /// proposal, an assertion or a disposition id that its file holds on two
    /// lines, the refusal is the refusal of the located check for that id.
    #[allow(clippy::disallowed_methods)]
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        provenance_core::validate_ideation_aggregate(IdeationAggregate {
            legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
            disposition_actor_ids: self.disposition_actor_ids,
            contributions: self.contributions,
            synthesis_packets: self.synthesis_packets,
            proposals: self.proposals,
            assertions: self.assertions,
            dispositions: self.dispositions,
        })
        .map_err(|error| {
            let located = error
                .downcast_ref::<DuplicateImmutableId>()
                .and_then(|duplicate| self.located(duplicate));
            located.unwrap_or(error)
        })
    }

    fn located(&self, duplicate: &DuplicateImmutableId) -> Option<anyhow::Error> {
        [
            &self.proposals.lines,
            &self.assertions.lines,
            &self.dispositions.lines,
        ]
        .into_iter()
        .find(|lines| lines.kind == duplicate.kind)?
        .check(|id| id == duplicate.id)
        .err()
    }
}

impl StateStore {
    /// The rows of the file of `T` in `scope`, with no records of the
    /// landing batches. A catch-up pass keeps this parse. The kept parse
    /// holds the lines and no result of a check, so each check runs again.
    pub(super) fn read_immutable_rows<T: ImmutableRecord>(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<ImmutableRows<T>> {
        let path = T::path(&self.layout, scope);
        let rows = shared(self, &path, || {
            Ok(read_jsonl_located::<T>(self, &path)?
                .into_iter()
                .map(|(record, at)| (record, at.line))
                .collect::<Vec<_>>())
        })?;
        Ok(ImmutableRows { path, rows })
    }

    /// The records of `T` in `scope` with the records of the landing batches
    /// over them. The ids are not checked.
    pub(super) fn listed<T: ImmutableRecord>(&self, scope: &ScopeId) -> anyhow::Result<Listed<T>> {
        self.with_repository_read(|held| {
            let (mut records, lines) = held.read_immutable_rows::<T>(scope)?.split();
            for batch in held.list_ideation_landings(scope)? {
                overlay_records(&mut records, T::landed(batch), T::id);
            }
            Ok(Listed { records, lines })
        })
    }
}
