//! One write/read size invariant for canonical resource records.
//!
//! The supported resource member and page readers account one stored byte
//! count per record and refuse a record above the budget in
//! [`crate::cache::read::page`]. A write that the store accepts must never
//! publish a record those readers then refuse, so every resource write
//! measures the record the same way and refuses the mutation before the
//! shard is published.
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::model::projection_row::ColumnValue;
use provenance_core::model::ProjectionRow;
use provenance_macros::rule;
use serde::Serialize;

/// The shared resource read budget: the largest stored record byte count
/// that every supported resource member and page read returns.
pub(crate) fn resource_record_bytes() -> usize {
    crate::cache::read::page::RESOURCE_RECORD_BYTES
}

/// The canonical row budget the shared query engine enforces, which the
/// discussion page readers also apply to messages.
pub(crate) fn record_bytes() -> usize {
    crate::cache::read::page::RECORD_BYTES
}

/// The byte accounting one supported read applies to one record kind.
pub(crate) trait ReadBudget: Serialize {
    /// The stored bytes the supported reads account for this record.
    fn read_bytes(&self) -> anyhow::Result<usize>;

    /// The read budget this record kind's supported reads enforce.
    fn read_budget() -> usize {
        resource_record_bytes()
    }
}

/// The byte count `length(CAST(column AS BLOB))` reports for one column
/// value: SQLite renders a null as absent, an integer as its decimal text,
/// and text as its UTF-8 bytes. No supported resource column holds a real.
fn column_bytes(value: &ColumnValue) -> usize {
    match value {
        ColumnValue::Null => 0,
        ColumnValue::Text(text) => text.len(),
        ColumnValue::Integer(integer) => integer.to_string().len(),
        ColumnValue::Real(real) => real.to_string().len(),
    }
}

/// Refuses the mutation before publication when the record's stored byte
/// count exceeds the byte budget that the supported reads return.
#[rule("rule_accepted_writes_stay_readable")]
pub(crate) fn ensure_within_read_budget<T: ReadBudget>(record: &T) -> anyhow::Result<()> {
    let size = record.read_bytes()?;
    let budget = T::read_budget();
    if size > budget {
        return Err(SourceFailure::wrap(
            WriteFailure::RecordTooLarge,
            anyhow::anyhow!(
                "record serializes to {size} bytes; the supported reads return at most {budget} bytes"
            ),
        ));
    }
    Ok(())
}

/// Applies [`ensure_within_read_budget`] to every record of a replaced or
/// imported shard.
pub(crate) fn ensure_slice_within_read_budget<T: ReadBudget>(records: &[T]) -> anyhow::Result<()> {
    for record in records {
        ensure_within_read_budget(record)?;
    }
    Ok(())
}

macro_rules! projection_budget {
    ($($kind:ty),+ $(,)?) => {
        $(
            impl ReadBudget for $kind {
                fn read_bytes(&self) -> anyhow::Result<usize> {
                    Ok(self.row()?.iter().map(column_bytes).sum())
                }
            }
        )+
    };
}

macro_rules! payload_budget {
    ($($kind:ty),+ $(,)?) => {
        $(
            impl ReadBudget for $kind {
                fn read_bytes(&self) -> anyhow::Result<usize> {
                    Ok(serde_json::to_vec(self)?.len())
                }
            }
        )+
    };
}

projection_budget!(
    provenance_core::Source,
    provenance_core::Requirement,
    provenance_core::Resolution,
    provenance_core::Rule,
    provenance_core::Domain,
    provenance_core::Boundary,
    provenance_core::Topic,
    provenance_core::Question,
    provenance_core::VerificationBinding,
);

payload_budget!(
    provenance_core::Thread,
    provenance_core::Contribution,
    provenance_core::SynthesisPacket,
    provenance_core::ProposalCard,
    provenance_core::AssertionRecord,
    provenance_core::DispositionRecord,
);

impl ReadBudget for provenance_core::Message {
    fn read_bytes(&self) -> anyhow::Result<usize> {
        Ok(serde_json::to_vec(self)?.len())
    }
    /// The discussion and document page readers enforce the smaller
    /// canonical row budget on messages, so a message write honors it.
    fn read_budget() -> usize {
        record_bytes()
    }
}
