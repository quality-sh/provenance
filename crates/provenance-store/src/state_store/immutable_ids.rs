//! The ideation records whose ids are immutable: proposals, assertions and
//! dispositions.
//!
//! Each read of the file of one of these families comes here. A hand edit or
//! a branch merge can put one id on two lines. The read then refuses with
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
use std::collections::BTreeSet;

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

impl StateStore {
    /// The records of the file of `T` in `scope`, with no records of the
    /// landing batches. An id on two lines refuses at the second line.
    pub(super) fn read_immutable<T: ImmutableRecord>(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<T>> {
        let path = T::path(&self.layout, scope);
        shared(self, &path, || unique_ids(read_jsonl_located(self, &path)?))
    }

    /// [`Self::read_immutable`] through this store, which holds the section.
    pub(super) fn read_immutable_held<T: ImmutableRecord>(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<T>> {
        let path = T::path(&self.layout, scope);
        unique_ids(read_jsonl_located_held(self, &path)?)
    }
}

fn unique_ids<T: ImmutableRecord>(rows: Vec<(T, ShardLine)>) -> anyhow::Result<Vec<T>> {
    let mut seen = BTreeSet::new();
    for (record, at) in &rows {
        if !seen.insert(record.id()) {
            return Err(crate::refusal::state_fault(
                at,
                format!("duplicate immutable {} id {}", T::KIND, record.id()),
            ));
        }
    }
    Ok(rows.into_iter().map(|(record, _)| record).collect())
}
