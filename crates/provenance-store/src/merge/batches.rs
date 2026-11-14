//! The records that the batch rows of a shard hold, and the conflicts between
//! the records of the batches that the two sides of a merge add.
//!
//! A read applies the batches in file order, and a later batch replaces a
//! record of an earlier batch. Thus a record of a batch is a record as a row
//! of its own family is: when the two sides change it differently, the merge
//! reports a conflict and keeps our record.

use super::{CanonicalRecord, MergeConflict, MergeConflictKind};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The lists of records that one batch row holds, and the field that
/// identifies a record in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchRecords {
    lists: &'static [&'static str],
    key: &'static str,
}

impl BatchRecords {
    /// The records of a landing batch in `ideation/landings.jsonl`.
    pub const LANDINGS: Self = Self {
        lists: &[
            "contributions",
            "synthesis_packets",
            "proposals",
            "assertions",
            "dispositions",
        ],
        key: "id",
    };

    /// The field of a batch row that holds each list of records.
    #[must_use]
    pub const fn lists(self) -> &'static [&'static str] {
        self.lists
    }

    /// The field that identifies a record in a list.
    #[must_use]
    pub const fn key(self) -> &'static str {
        self.key
    }

    /// The record that a read of `rows` gives for each id of `list`: the
    /// record of the last row that holds the id.
    fn read<'a>(
        self,
        list: &str,
        rows: impl IntoIterator<Item = &'a CanonicalRecord>,
    ) -> anyhow::Result<BTreeMap<String, CanonicalRecord>> {
        let mut read = BTreeMap::new();
        for row in rows {
            for record in list_records(list, row)? {
                read.insert(
                    super::string_id(self.key, record)?.to_owned(),
                    record.clone(),
                );
            }
        }
        Ok(read)
    }
}

/// The records of `list` in one batch row. A row with no such field holds no
/// records of the list.
fn list_records<'a>(list: &str, row: &'a CanonicalRecord) -> anyhow::Result<&'a [Value]> {
    match row.get(list) {
        None => Ok(&[]),
        Some(Value::Array(records)) => Ok(records),
        Some(_) => Err(crate::refusal::fault!("batch field {list} is not a list")),
    }
}

/// The record conflicts of a batch merge, and the batches of theirs that the
/// merged file does not hold.
#[derive(Debug, Default)]
pub(super) struct RecordConflicts {
    pub(super) conflicts: Vec<MergeConflict>,
    pub(super) left_out: BTreeSet<String>,
}

/// Compares the records of the batches that ours adds with the records of
/// the batches that theirs adds.
///
/// A record is the record that a read of the added batches of a side gives.
/// When both sides give a record for one id and the two records are not
/// equal, the merge reports a conflict that names the list and the id. The
/// conflict is `add_add` when no batch of the base holds the id, and
/// `divergent_edit` with the base record when one does. Each batch that only
/// theirs adds and that holds the id with a record different from ours is
/// left out of the merged file, so that a read of the file gives our record.
/// The conflicts are in the order of the lists, then of the ids.
pub(super) fn record_conflicts(
    records: BatchRecords,
    [base, ours, theirs]: [&[(String, CanonicalRecord)]; 3],
) -> anyhow::Result<RecordConflicts> {
    let base_keys: BTreeSet<&str> = base.iter().map(|(key, _)| key.as_str()).collect();
    let our_keys: BTreeSet<&str> = ours.iter().map(|(key, _)| key.as_str()).collect();
    let our_additions = additions(ours, &base_keys);
    let their_additions = additions(theirs, &base_keys);
    let mut result = RecordConflicts::default();
    for list in records.lists {
        let base_read = records.read(list, base.iter().map(|(_, row)| row))?;
        let our_read = records.read(list, our_additions.iter().map(|(_, row)| row))?;
        let their_read = records.read(list, their_additions.iter().map(|(_, row)| row))?;
        for (id, our_record) in &our_read {
            let Some(their_record) = their_read.get(id).filter(|theirs| *theirs != our_record)
            else {
                continue;
            };
            let base_record = base_read.get(id);
            result.conflicts.push(MergeConflict {
                kind: if base_record.is_none() {
                    MergeConflictKind::AddAdd
                } else {
                    MergeConflictKind::DivergentEdit
                },
                record_id: format!("{list}/{id}"),
                base: base_record.cloned(),
                ours: Some(our_record.clone()),
                theirs: Some(their_record.clone()),
            });
            for (key, row) in &their_additions {
                if !our_keys.contains(key.as_str())
                    && holds_other_record(records.key, list_records(list, row)?, id, our_record)
                {
                    result.left_out.insert(key.clone());
                }
            }
        }
    }
    Ok(result)
}

/// The rows of one side that no row of the base is equal to.
fn additions<'a>(
    rows: &'a [(String, CanonicalRecord)],
    base_keys: &BTreeSet<&str>,
) -> Vec<&'a (String, CanonicalRecord)> {
    rows.iter()
        .filter(|(key, _)| !base_keys.contains(key.as_str()))
        .collect()
}

/// Whether `records` hold a record with `id` that is not `ours`.
fn holds_other_record(key: &str, records: &[Value], id: &str, ours: &CanonicalRecord) -> bool {
    records
        .iter()
        .any(|record| record.get(key).and_then(Value::as_str) == Some(id) && record != ours)
}
