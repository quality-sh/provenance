//! The report of a conflict of the landing batches, and its text.

use super::{MergeConflict, MergeSide};

/// Which run of batches comes first in each place of a merged file, before a
/// common batch or after the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeOrder {
    /// The batches that only ours holds come first.
    OursFirst,
    /// The batches that only theirs holds come first.
    TheirsFirst,
}

impl std::fmt::Display for MergeOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OursFirst => "our batches first",
            Self::TheirsFirst => "their batches first",
        })
    }
}

/// The first check of the read that the merged file fails.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FailedCheck {
    /// The run of batches that comes first in the file that fails.
    pub order: MergeOrder,
    /// The side whose batch fails the check.
    pub side: MergeSide,
    /// The row of that batch in the file of its side, from 1. Empty lines
    /// are not rows.
    pub row: usize,
    /// The record of the batch that fails the check, as `<list>/<id>`.
    pub record_id: String,
    /// The refusal of the check.
    pub check: String,
}

/// The first common batch that ours and theirs do not hold in one sequence.
/// A common batch is a batch of the base or a batch that the two sides add.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SequenceDifference {
    /// The row of our common batch at that place, from 1, or `None` when
    /// ours holds no more common batches.
    pub ours: Option<usize>,
    /// The row of their common batch at that place, from 1, or `None` when
    /// theirs holds no more common batches.
    pub theirs: Option<usize>,
}

/// A batch of theirs that the merged file does not hold.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LeftOutBatch {
    /// The row of the batch in their file, from 1. Empty lines are not rows.
    pub row: usize,
    /// Each record of the batch, as `<list>/<id>`.
    pub record_ids: Vec<String>,
}

/// The text of a conflict of the landings file: each cause, the batches of
/// theirs that the file does not hold, and the next step.
pub(super) fn conflict_text(
    conflicts: &[MergeConflict],
    sequence: Option<&SequenceDifference>,
    failed_checks: &[FailedCheck],
    left_out: &[LeftOutBatch],
) -> String {
    let mut lines = vec![String::from(
        "the merge of the landing batches has a conflict, so the file holds the batches of our \
         side",
    )];
    if !conflicts.is_empty() {
        let ids: Vec<_> = conflicts
            .iter()
            .map(|conflict| conflict.record_id.as_str())
            .collect();
        lines.push(format!(
            "records that the two sides change differently: {}",
            ids.join(", ")
        ));
    }
    if let Some(sequence) = sequence {
        let place = |row: Option<usize>, side: &str| {
            row.map_or_else(|| format!("the end of {side}"), |row| format!("row {row} of {side}"))
        };
        lines.push(format!(
            "common batches that the two sides hold in different sequences, from {} and {}",
            place(sequence.ours, "ours"),
            place(sequence.theirs, "theirs")
        ));
    }
    if !failed_checks.is_empty() {
        let checks: Vec<_> = failed_checks
            .iter()
            .map(|failed| {
                format!(
                    "row {} of {} with {}: {}",
                    failed.row, failed.side, failed.order, failed.check
                )
            })
            .collect();
        lines.push(format!(
            "checks that the merged file fails: {}",
            checks.join("; ")
        ));
    }
    if !left_out.is_empty() {
        let batches: Vec<_> = left_out
            .iter()
            .map(|batch| {
                format!(
                    "row {} of theirs: {}",
                    batch.row,
                    batch.record_ids.join(", ")
                )
            })
            .collect();
        lines.push(format!(
            "batches of theirs that the file does not hold: {}",
            batches.join("; ")
        ));
    }
    lines.push(String::from(NEXT_STEP));
    lines.join("\n")
}

/// What a person can do after a conflict of the landings file.
const NEXT_STEP: &str = "Git keeps their version of the file as stage 3 of the path. When a \
    batch of theirs comes from a swarm run, `provenance swarm-backtrace land --replace \
    --run-dir <run>` lands the run again, and each contribution and synthesis packet of the run \
    then has the record of theirs. The landing refuses the run when the file holds a proposal, \
    an assertion or a disposition with an id of the run, or when an assertion of the file cites \
    a contribution or a synthesis packet that the run changes. No command of Provenance replaces \
    a proposal, an assertion or a disposition that the file holds, or a contribution or a \
    synthesis packet that an assertion of the file cites. After the change, mark the path as \
    resolved.";
