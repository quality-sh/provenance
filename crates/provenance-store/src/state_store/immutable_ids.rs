//! The ideation records whose ids are immutable: proposals, assertions and
//! dispositions.
//!
//! The list reader of each of these families, the ideation scope check and
//! the scope import read the file of the family here. A hand edit or a
//! branch merge can put one id on two lines. The read then refuses with
//! `invalid_state_file` at the second line, before a check over the records
//! that does not know the lines.

use super::message_ids::ShardLine;
use super::readers::{read_jsonl_located, read_jsonl_located_held};
use super::shared_parses::shared;
use super::{ProvenanceLayout, StateStore};
use crate::shards;
use camino::Utf8PathBuf;
use provenance_core::{AssertionRecord, DispositionRecord, ProposalCard, ScopeId};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

/// A family of ideation records whose ids are immutable.
pub(super) trait ImmutableRecord: DeserializeOwned + Clone + Send + 'static {
    /// The name of the family in a refusal.
    const KIND: &'static str;
    /// The file of the family in `scope`.
    fn path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf;
    fn id(&self) -> &str;
}

macro_rules! immutable_record {
    ($record:ty, $kind:literal, $path:ident) => {
        impl ImmutableRecord for $record {
            const KIND: &'static str = $kind;
            fn path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                shards::$path(layout, scope)
            }
            fn id(&self) -> &str {
                self.id.as_str()
            }
        }
    };
}

immutable_record!(ProposalCard, "proposal", proposal_cards_path);
immutable_record!(AssertionRecord, "assertion", assertion_records_path);
immutable_record!(DispositionRecord, "disposition", dispositions_path);

/// The rows of the file of one family, each with its line, before the check
/// of the ids.
pub(super) struct ImmutableRows<T> {
    path: Utf8PathBuf,
    rows: Vec<(T, usize)>,
}

impl<T: ImmutableRecord> ImmutableRows<T> {
    /// The records, or `invalid_state_file` at the second line of an id
    /// that the file holds two times. The message names the first line and
    /// the second line of the id.
    pub(super) fn unique(self) -> anyhow::Result<Vec<T>> {
        let mut first_lines = BTreeMap::new();
        for (record, line) in &self.rows {
            if let Some(first) = first_lines.insert(record.id(), *line) {
                let (kind, id) = (T::KIND, record.id());
                return Err(crate::refusal::state_fault(
                    &ShardLine::new(&self.path, *line),
                    format!(
                        "duplicate immutable {kind} id {id} at line {first} and at line {line}"
                    ),
                ));
            }
        }
        Ok(self.rows.into_iter().map(|(record, _)| record).collect())
    }
}

impl StateStore {
    /// The rows of the file of `T` in `scope`, with no records of the
    /// landing batches. A catch-up pass keeps this parse. The kept parse
    /// holds the lines and no result of the check, so each read checks the
    /// ids again.
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

    /// The records of the file of `T` in `scope`, with no records of the
    /// landing batches. An id on two lines refuses at the second line.
    pub(super) fn read_immutable<T: ImmutableRecord>(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<T>> {
        self.read_immutable_rows(scope)?.unique()
    }

    /// [`Self::read_immutable`] through this store, which holds the section.
    ///
    /// [`StateStore::import_scope`] reads the stored files here. The porcelain
    /// import reads them before, in its review-portability check and its
    /// snapshot, so on that path an id on two lines refuses there first.
    pub(super) fn read_immutable_held<T: ImmutableRecord>(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<T>> {
        let path = T::path(&self.layout, scope);
        let rows = read_jsonl_located_held::<T>(self, &path)?
            .into_iter()
            .map(|(record, at)| (record, at.line))
            .collect();
        ImmutableRows { path, rows }.unique()
    }
}
