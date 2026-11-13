//! The located refusal of an immutable ideation id on two lines of its file.
//!
//! A hand edit or a branch merge can put one id on two lines of the file of
//! a contribution, a synthesis packet, a proposal, an assertion or a
//! disposition. Each aggregate check of the ideation records in the store
//! goes through [`StateStore::check_ideation_aggregate`], and the ideation
//! scope check also refuses a proposal, assertion or disposition id on two
//! lines of its file. Both refuse with `invalid_state_file` at the second
//! line, and the message names the two lines.
//!
//! The records that the checks get have no lines: each list of a kind joins
//! the rows of its file, the landings and the record of the write. So the
//! refusal path reads the file of the kind again with the same typed parse
//! and the same id as the list reader. The two reads give the same rows when
//! the file has the same bytes.

use super::readers::read_jsonl_located;
use super::StateStore;
use crate::families::RecordFamily;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, DuplicateImmutableId, IdeationAggregate,
    IdeationRecordKind, ProposalCard, ScopeId, SynthesisPacket,
};
use serde::de::DeserializeOwned;

impl StateStore {
    /// [`provenance_core::validate_ideation_aggregate`] over `aggregate`, the
    /// ideation records of `scope`. A duplicate id has the refusal of
    /// [`StateStore::duplicate_id_fault`].
    pub(super) fn check_ideation_aggregate(
        &self,
        scope: &ScopeId,
        aggregate: IdeationAggregate<'_>,
    ) -> anyhow::Result<()> {
        provenance_core::validate_ideation_aggregate(aggregate).map_err(|error| {
            match error.downcast::<DuplicateImmutableId>() {
                Ok(duplicate) => self.duplicate_id_fault(scope, duplicate),
                Err(error) => error,
            }
        })
    }

    /// `invalid_state_file` at the second line of the file of
    /// `duplicate.kind` in `scope` with the id `duplicate.id`, with a message
    /// that names the first line and the second line.
    ///
    /// The refusal is `duplicate` with no file and no line only when the file
    /// does not have the bytes that the check read: a view never changes, and
    /// a store with a section holds the publication section, so only a
    /// program outside Provenance can change the file between the two reads.
    /// A read-only store takes no lock, so a write of another process can
    /// also change it.
    pub(super) fn duplicate_id_fault(
        &self,
        scope: &ScopeId,
        duplicate: DuplicateImmutableId,
    ) -> anyhow::Error {
        let located = match duplicate.kind {
            IdeationRecordKind::Contribution => {
                self.located_duplicate::<Contribution>(scope, &duplicate)
            }
            IdeationRecordKind::SynthesisPacket => {
                self.located_duplicate::<SynthesisPacket>(scope, &duplicate)
            }
            IdeationRecordKind::Proposal => {
                self.located_duplicate::<ProposalCard>(scope, &duplicate)
            }
            IdeationRecordKind::Assertion => {
                self.located_duplicate::<AssertionRecord>(scope, &duplicate)
            }
            IdeationRecordKind::Disposition => {
                self.located_duplicate::<DispositionRecord>(scope, &duplicate)
            }
        };
        located.unwrap_or_else(|| duplicate.into())
    }

    /// The located refusal of `duplicate` from the records of the type `T`
    /// in its file, each with its line. `None` when the read fails or does
    /// not show the id on two lines.
    fn located_duplicate<T: RecordFamily + DeserializeOwned>(
        &self,
        scope: &ScopeId,
        duplicate: &DuplicateImmutableId,
    ) -> Option<anyhow::Error> {
        let path = T::FAMILY.shard_path(&self.layout, scope);
        let rows = read_jsonl_located::<T>(self, &path).ok()?;
        let mut lines = rows
            .into_iter()
            .filter(|(record, _)| record.record_id() == duplicate.id)
            .map(|(_, at)| at);
        let first = lines.next()?;
        let second = lines.next()?;
        let message = format!("{duplicate} at line {} and at line {}", first.line, second.line);
        Some(crate::refusal::state_fault(&second, message))
    }
}
