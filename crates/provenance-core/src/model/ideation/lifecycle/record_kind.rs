//! The five kinds of ideation record whose id is immutable.

use super::AssertionRecord;
use crate::model::{Contribution, DispositionRecord, ProposalCard, SynthesisPacket};

/// The kind of an ideation record whose id is immutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeationRecordKind {
    /// A [`Contribution`].
    Contribution,
    /// A [`SynthesisPacket`].
    SynthesisPacket,
    /// A [`ProposalCard`].
    Proposal,
    /// An [`AssertionRecord`].
    Assertion,
    /// A [`DispositionRecord`].
    Disposition,
}

impl std::fmt::Display for IdeationRecordKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Contribution => "contribution",
            Self::SynthesisPacket => "synthesis packet",
            Self::Proposal => "proposal",
            Self::Assertion => "assertion",
            Self::Disposition => "disposition",
        })
    }
}

/// An ideation record of one [`IdeationRecordKind`], with its immutable id.
pub trait IdeationRecord {
    /// The kind of each record of this type.
    const KIND: IdeationRecordKind;

    /// The immutable id of the record.
    fn immutable_id(&self) -> &str;
}

macro_rules! ideation_records {
    ($($record:ty => $kind:ident),* $(,)?) => {$(
        impl IdeationRecord for $record {
            const KIND: IdeationRecordKind = IdeationRecordKind::$kind;

            fn immutable_id(&self) -> &str {
                self.id.as_str()
            }
        }
    )*};
}

ideation_records! {
    Contribution => Contribution,
    SynthesisPacket => SynthesisPacket,
    ProposalCard => Proposal,
    AssertionRecord => Assertion,
    DispositionRecord => Disposition,
}

/// The refusal of [`super::validate_ideation_aggregate`] when two records of
/// one kind have the same immutable id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("duplicate immutable {kind} id {id}")]
pub struct DuplicateImmutableId {
    /// The kind of the two records.
    pub kind: IdeationRecordKind,
    /// The id that the two records have.
    pub id: String,
}
