//! The merge of the landing batches of `ideation/landings.jsonl`.
//!
//! A read applies the batches in the order of the file, so the merge cannot
//! decide record by record. It makes two candidate files: our batches, then
//! the batches that only theirs adds; and their batches, then the batches
//! that only ours adds. The merge is clean when each candidate passes the
//! checks of the read and the two candidates read the same record for each
//! id. Then the merged file is the first candidate. If not, the merge is a
//! conflict and the file is our file.

use super::{CanonicalRecord, MergeConflict, MergeConflictKind, MergeOutcome, MergeSide};
use crate::state_store::landing_read::{IdeationRecords, Records};
use crate::state_store::IdeationLandingBatch;
use provenance_core::DuplicateImmutableId;
use provenance_macros::rule;
use std::collections::BTreeSet;

/// The order of the batches of a candidate file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeOrder {
    /// Our batches, then the batches that only theirs adds.
    OursThenTheirs,
    /// Their batches, then the batches that only ours adds.
    TheirsThenOurs,
}

impl std::fmt::Display for MergeOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OursThenTheirs => "ours, theirs",
            Self::TheirsThenOurs => "theirs, ours",
        })
    }
}

/// The first check of the read that a candidate file fails.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FailedCheck {
    /// The order of the candidate.
    pub order: MergeOrder,
    /// The side whose batch fails the check.
    pub side: MergeSide,
    /// The line of that batch in the file of its side, from 1.
    pub line: usize,
    /// The record of the batch that fails the check, as `<list>/<id>`.
    pub record_id: String,
    /// The refusal of the check.
    pub check: String,
}

/// A batch that only theirs adds and that the merged file does not hold.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LeftOutBatch {
    /// The line of the batch in their file, from 1.
    pub line: usize,
    /// Each record of the batch, as `<list>/<id>`.
    pub record_ids: Vec<String>,
}

/// The records of each field of a landing batch, by id.
type Lists = [(&'static str, Records); 5];

/// One row of a side, with its key and its line in the file of the side.
#[derive(Clone, Copy)]
struct Row<'a> {
    side: MergeSide,
    line: usize,
    key: &'a str,
    record: &'a CanonicalRecord,
}

/// Merges the batch rows of the three sides, each row with its key.
#[rule("rule_landing_batch_merge_conflict")]
pub(super) fn merge_batches(
    [base, ours, theirs]: [&[(String, CanonicalRecord)]; 3],
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    let [base, ours, theirs] = [
        (MergeSide::Base, base),
        (MergeSide::Ours, ours),
        (MergeSide::Theirs, theirs),
    ]
    .map(|(side, rows)| numbered(side, rows));
    let only_ours = only_one_side(&ours, [&base, &theirs]);
    let only_theirs = only_one_side(&theirs, [&base, &ours]);
    let first = Candidate::read(MergeOrder::OursThenTheirs, ours.iter().chain(&only_theirs))?;
    let second = Candidate::read(MergeOrder::TheirsThenOurs, theirs.iter().chain(&only_ours))?;
    let conflicts = read_conflicts(&first.lists, &second.lists, [&base, &ours, &theirs])?;
    let failed_checks: Vec<_> = [first.failed, second.failed]
        .into_iter()
        .flatten()
        .collect();
    if conflicts.is_empty() && failed_checks.is_empty() {
        return Ok(MergeOutcome::Clean {
            records: first.rows,
        });
    }
    Ok(MergeOutcome::Conflicted {
        conflicts,
        failed_checks,
        left_out: only_theirs
            .iter()
            .map(|row| {
                Ok(LeftOutBatch {
                    line: row.line,
                    record_ids: record_ids(&read([row])?)?,
                })
            })
            .collect::<anyhow::Result<_>>()?,
        partial: ours.iter().map(|row| row.record.clone()).collect(),
    })
}

fn numbered(side: MergeSide, rows: &[(String, CanonicalRecord)]) -> Vec<Row<'_>> {
    rows.iter()
        .enumerate()
        .map(|(index, (key, record))| Row {
            side,
            line: index + 1,
            key,
            record,
        })
        .collect()
}

/// The rows of `side` that no row of the other two sides is the same batch as.
fn only_one_side<'a>(side: &[Row<'a>], others: [&[Row<'a>]; 2]) -> Vec<Row<'a>> {
    let held: BTreeSet<&str> = others
        .iter()
        .flat_map(|rows| rows.iter())
        .map(|row| row.key)
        .collect();
    side.iter()
        .filter(|row| !held.contains(row.key))
        .copied()
        .collect()
}

/// A candidate file: its rows, its read, and the first check that it fails.
struct Candidate {
    rows: Vec<CanonicalRecord>,
    lists: Lists,
    failed: Option<FailedCheck>,
}

impl Candidate {
    /// Reads the rows in their order. Each batch goes through the checks of
    /// the read until one check fails, and the read applies each batch.
    fn read<'r, 'a: 'r>(
        order: MergeOrder,
        rows: impl IntoIterator<Item = &'r Row<'a>>,
    ) -> anyhow::Result<Self> {
        let mut records = IdeationRecords::default();
        let mut candidate = Self {
            rows: Vec::new(),
            lists: Default::default(),
            failed: None,
        };
        for row in rows {
            let batch = batch(row)?;
            if candidate.failed.is_none() {
                let duplicate = |id: DuplicateImmutableId| crate::refusal::fault!("{id}");
                if let Err(refusal) = records.check(&batch, duplicate) {
                    candidate.failed = Some(FailedCheck {
                        order,
                        side: row.side,
                        line: row.line,
                        record_id: refusal.record_id,
                        check: refusal.error.to_string(),
                    });
                }
            }
            records.apply(batch);
            candidate.rows.push(row.record.clone());
        }
        candidate.lists = records.lists()?;
        Ok(candidate)
    }
}

fn batch(row: &Row<'_>) -> anyhow::Result<IdeationLandingBatch> {
    serde_json::from_value(row.record.clone()).map_err(|error| {
        crate::refusal::fault!(
            "line {} of {} is not a landing batch: {error}",
            row.line,
            row.side
        )
    })
}

/// The read of the rows with no check.
fn read<'r, 'a: 'r>(
    rows: impl IntoIterator<Item = &'r Row<'a>>,
) -> anyhow::Result<IdeationRecords> {
    let mut records = IdeationRecords::default();
    for row in rows {
        records.apply(batch(row)?);
    }
    Ok(records)
}

/// Each record of the read, as `<list>/<id>`, in the order of the lists and
/// then of the ids.
fn record_ids(records: &IdeationRecords) -> serde_json::Result<Vec<String>> {
    Ok(records
        .lists()?
        .iter()
        .flat_map(|(list, records)| records.keys().map(move |id| format!("{list}/{id}")))
        .collect())
}

/// A conflict for each record that the two candidates read differently, with
/// the record that the base, our file and their file read.
fn read_conflicts(
    first: &Lists,
    second: &Lists,
    [base, ours, theirs]: [&[Row<'_>]; 3],
) -> anyhow::Result<Vec<MergeConflict>> {
    let reads = [
        read(base)?.lists()?,
        read(ours)?.lists()?,
        read(theirs)?.lists()?,
    ];
    let mut conflicts = Vec::new();
    for (index, ((list, first), (_, second))) in first.iter().zip(second).enumerate() {
        let ids: BTreeSet<&String> = first.keys().chain(second.keys()).collect();
        for id in ids {
            if first.get(id) == second.get(id) {
                continue;
            }
            let record = |side: usize| reads[side][index].1.get(id).cloned();
            let (base, ours, theirs) = (record(0), record(1), record(2));
            conflicts.push(MergeConflict {
                kind: match (&base, &ours, &theirs) {
                    (None, _, _) => MergeConflictKind::AddAdd,
                    (Some(_), Some(_), Some(_)) => MergeConflictKind::DivergentEdit,
                    (Some(_), _, _) => MergeConflictKind::DeleteModify,
                },
                record_id: format!("{list}/{id}"),
                base,
                ours,
                theirs,
            });
        }
    }
    Ok(conflicts)
}

/// The text of a conflict of the landings file: what the file holds, what
/// the merge left out, and the next step.
pub(super) fn conflict_text(
    conflicts: &[MergeConflict],
    failed_checks: &[FailedCheck],
    left_out: &[LeftOutBatch],
) -> String {
    let mut lines = vec![String::from(
        "the merge of the landing batches depends on the order of the two sides, so the \
         file holds the batches of our side",
    )];
    if !conflicts.is_empty() {
        let ids: Vec<_> = conflicts
            .iter()
            .map(|conflict| conflict.record_id.as_str())
            .collect();
        lines.push(format!(
            "records that the two orders read differently: {}",
            ids.join(", ")
        ));
    }
    if !failed_checks.is_empty() {
        let checks: Vec<_> = failed_checks
            .iter()
            .map(|failed| {
                format!(
                    "line {} of {} in the order {}: {}",
                    failed.line, failed.side, failed.order, failed.check
                )
            })
            .collect();
        lines.push(format!("checks that an order fails: {}", checks.join("; ")));
    }
    if !left_out.is_empty() {
        let batches: Vec<_> = left_out
            .iter()
            .map(|batch| {
                format!(
                    "line {} of theirs: {}",
                    batch.line,
                    batch.record_ids.join(", ")
                )
            })
            .collect();
        lines.push(format!(
            "batches of theirs that the file does not hold: {}",
            batches.join("; ")
        ));
    }
    lines.push(String::from(
        "Git keeps their version of the file as stage 3 of the path. To add a contribution or \
         a synthesis packet of theirs, land their swarm run again with `provenance \
         swarm-backtrace land --replace --run-dir <run>`, and then mark the path as resolved. \
         A landing cannot replace a proposal, an assertion or a disposition.",
    ));
    lines.join("\n")
}
