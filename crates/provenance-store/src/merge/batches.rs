//! The merge of the landing batches of `ideation/landings.jsonl`.
//!
//! A common batch is a batch of the base, or a batch that the two sides add
//! with equal rows. Each other batch is held by one side only, and that side
//! changes each record of the batch. A record that only one side changes
//! gets the read of that side. A record that the two sides change is a
//! conflict when our read of it and their read of it are different.
//!
//! The merged file keeps the sequence of each side: the common batches, and
//! before each one and after the last one, the batches that only ours holds
//! there and the batches that only theirs holds there. Then the last batch
//! of the file that holds a record is the last such batch of one side, so the
//! read of the file gives each record as the rule above gives it. The file
//! must also pass the checks of the read with either run first.

use super::batch_report::{FailedCheck, LeftOutBatch, MergeOrder, SequenceDifference};
use super::{CanonicalRecord, MergeConflict, MergeConflictKind, MergeOutcome, MergeSide};
use crate::state_store::landing_read::IdeationRecords;
use crate::state_store::IdeationLandingBatch;
use provenance_macros::rule;
use std::collections::BTreeSet;

/// One row of a side: its key, its batch, and its number in the file of the
/// side, from 1.
struct Row<'a> {
    side: MergeSide,
    number: usize,
    key: &'a str,
    record: &'a CanonicalRecord,
    batch: IdeationLandingBatch,
}

/// The ids of the records of a batch or a read, for each field of a landing
/// batch, in the order of the fields.
type Ids = [BTreeSet<String>; 5];

/// Merges the batch rows of the three sides, each row with its key.
#[rule("rule_landing_batch_merge_conflict")]
pub(super) fn merge_batches(
    [base, ours, theirs]: [&[(String, CanonicalRecord)]; 3],
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    let base = rows(MergeSide::Base, base)?;
    let ours = rows(MergeSide::Ours, ours)?;
    let theirs = rows(MergeSide::Theirs, theirs)?;
    let common = common_batches(&base, &ours, &theirs);
    let conflicts = record_conflicts([&base, &ours, &theirs], &common)?;
    let sequence = sequence_difference(&ours, &theirs, &common);
    let (failed_checks, merged) = if sequence.is_none() {
        let ours_first = merged_file(&ours, &theirs, &common);
        let theirs_first = merged_file(&theirs, &ours, &common);
        let failed_checks: Vec<_> = [
            failed_check(MergeOrder::OursFirst, &ours_first),
            failed_check(MergeOrder::TheirsFirst, &theirs_first),
        ]
        .into_iter()
        .flatten()
        .collect();
        let merged = ours_first.iter().map(|row| row.record.clone()).collect();
        (failed_checks, merged)
    } else {
        (Vec::new(), Vec::new())
    };
    if conflicts.is_empty() && sequence.is_none() && failed_checks.is_empty() {
        return Ok(MergeOutcome::Clean { records: merged });
    }
    let held: BTreeSet<&str> = ours.iter().map(|row| row.key).collect();
    Ok(MergeOutcome::Conflicted {
        conflicts,
        sequence,
        failed_checks,
        left_out: theirs
            .iter()
            .filter(|row| !held.contains(row.key))
            .map(|row| {
                Ok(LeftOutBatch {
                    row: row.number,
                    record_ids: record_ids(&read([row]))?,
                })
            })
            .collect::<serde_json::Result<_>>()?,
        partial: ours.iter().map(|row| row.record.clone()).collect(),
    })
}

/// Parses each row of `side` as a landing batch.
fn rows(side: MergeSide, rows: &[(String, CanonicalRecord)]) -> anyhow::Result<Vec<Row<'_>>> {
    rows.iter()
        .enumerate()
        .map(|(index, (key, record))| {
            let number = index + 1;
            let batch = serde_json::from_value(record.clone()).map_err(|error| {
                crate::refusal::fault!("row {number} of {side} is not a landing batch: {error}")
            })?;
            Ok(Row {
                side,
                number,
                key,
                record,
                batch,
            })
        })
        .collect()
}

/// The keys of the batches of the base and of the batches that the two
/// sides add with equal rows.
fn common_batches<'a>(base: &[Row<'a>], ours: &[Row<'a>], theirs: &[Row<'a>]) -> BTreeSet<&'a str> {
    let theirs: BTreeSet<&str> = theirs.iter().map(|row| row.key).collect();
    let added_by_two = ours
        .iter()
        .map(|row| row.key)
        .filter(|key| theirs.contains(key));
    base.iter().map(|row| row.key).chain(added_by_two).collect()
}

/// A conflict for each record that each side changes and that our read and
/// their read give differently, in the order of the fields and then of the
/// ids, with the record that the base, our file and their file read. A side
/// that changes a record reads it, so the kind is `add_add` or
/// `divergent_edit`.
fn record_conflicts(
    [base, ours, theirs]: [&[Row<'_>]; 3],
    common: &BTreeSet<&str>,
) -> serde_json::Result<Vec<MergeConflict>> {
    let reads = [
        read(base).lists()?,
        read(ours).lists()?,
        read(theirs).lists()?,
    ];
    let [our_changes, their_changes] = [changes(ours, common)?, changes(theirs, common)?];
    let mut conflicts = Vec::new();
    for (index, (list, _)) in reads[0].iter().enumerate() {
        for id in our_changes[index].intersection(&their_changes[index]) {
            let record = |side: usize| reads[side][index].1.get(id).cloned();
            let (base, ours, theirs) = (record(0), record(1), record(2));
            if ours == theirs {
                continue;
            }
            conflicts.push(MergeConflict {
                kind: if base.is_none() {
                    MergeConflictKind::AddAdd
                } else {
                    MergeConflictKind::DivergentEdit
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

/// The records that a side changes: each record of each batch of the side
/// that is not a common batch.
fn changes(side: &[Row<'_>], common: &BTreeSet<&str>) -> serde_json::Result<Ids> {
    let mut ids = Ids::default();
    for row in side.iter().filter(|row| !common.contains(row.key)) {
        for (changed, (_, records)) in ids.iter_mut().zip(read([row]).lists()?) {
            changed.extend(records.into_keys());
        }
    }
    Ok(ids)
}

/// The first place where ours and theirs hold their common batches
/// differently, or `None` when the two sides hold them in one sequence.
fn sequence_difference(
    ours: &[Row<'_>],
    theirs: &[Row<'_>],
    common: &BTreeSet<&str>,
) -> Option<SequenceDifference> {
    let mut ours = ours.iter().filter(|row| common.contains(row.key));
    let mut theirs = theirs.iter().filter(|row| common.contains(row.key));
    loop {
        let (our_row, their_row) = (ours.next(), theirs.next());
        if our_row.is_none() && their_row.is_none() {
            return None;
        }
        if our_row.map(|row| row.key) != their_row.map(|row| row.key) {
            return Some(SequenceDifference {
                ours: our_row.map(|row| row.number),
                theirs: their_row.map(|row| row.number),
            });
        }
    }
}

/// The batches of the two sides in one file that keeps the sequence of each
/// side. The file holds the common batches in their sequence, and before
/// each one and after the last one, the run of `first` and then the run of
/// `second` that its side holds there. The two sides must hold the common
/// batches in one sequence.
#[rule("rule_landing_batch_merge")]
fn merged_file<'r, 'a>(
    first: &'r [Row<'a>],
    second: &'r [Row<'a>],
    common: &BTreeSet<&str>,
) -> Vec<&'r Row<'a>> {
    let is_common = |row: &Row<'_>| common.contains(row.key);
    let mut first_runs = first.split(is_common);
    let mut second_runs = second.split(is_common);
    let mut common_rows = first.iter().filter(|&row| is_common(row));
    let mut file = Vec::new();
    loop {
        file.extend(first_runs.next().into_iter().flatten());
        file.extend(second_runs.next().into_iter().flatten());
        match common_rows.next() {
            Some(row) => file.push(row),
            None => return file,
        }
    }
}

/// The first check of the read that `file` fails.
fn failed_check(order: MergeOrder, file: &[&Row<'_>]) -> Option<FailedCheck> {
    let mut records = IdeationRecords::default();
    for row in file {
        if let Err(refusal) = records.check(&row.batch) {
            return Some(FailedCheck {
                order,
                side: row.side,
                row: row.number,
                record_id: refusal.record_id,
                check: refusal.cause.to_string(),
            });
        }
        records.apply(row.batch.clone());
    }
    None
}

/// The read of the rows with no check.
fn read<'r, 'a: 'r>(rows: impl IntoIterator<Item = &'r Row<'a>>) -> IdeationRecords {
    let mut records = IdeationRecords::default();
    for row in rows {
        records.apply(row.batch.clone());
    }
    records
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
