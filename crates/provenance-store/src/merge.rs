use crate::jsonl::numbered_lines;
use provenance_core::protocol::read_failure::UnparsableStateFile;
use provenance_macros::rule;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod review_cycle;
pub mod validation;

pub use review_cycle::review_cycle_conflicts;
pub use validation::{changed_statement_diagnostics, validate_merged_records, ShardFamily};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeConflictKind {
    /// Both sides added the same id with different content. There is no base
    /// pre-image to compare against, so neither side can be called the mover.
    AddAdd,
    /// Both sides changed a record that existed in the base, differently.
    DivergentEdit,
    /// One side deleted a record the other side changed.
    DeleteModify,
    /// Each side added a review submission of one record with the same
    /// review cycle.
    SameReviewCycle,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MergeConflict {
    pub kind: MergeConflictKind,
    pub record_id: String,
    /// The base pre-image, so a reader can see what both sides moved away
    /// from. `None` for an add/add clash, where the base held nothing.
    pub base: Option<CanonicalRecord>,
    pub ours: Option<Value>,
    pub theirs: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MergeOutcome<T> {
    Clean {
        records: T,
    },
    Conflicted {
        conflicts: Vec<MergeConflict>,
        partial: T,
    },
}

impl<T> MergeOutcome<T> {
    pub fn unwrap_clean(self) -> T {
        match self {
            Self::Clean { records } => records,
            Self::Conflicted { .. } => panic!("expected clean merge"),
        }
    }

    pub fn unwrap_conflicts(self) -> Vec<MergeConflict> {
        match self {
            Self::Clean { .. } => panic!("expected conflicted merge"),
            Self::Conflicted { conflicts, .. } => conflicts,
        }
    }
}

pub type CanonicalRecord = Value;

/// One stored input row kept beside the record parsed from it.
///
/// A merge never authors record content: every merged record is one side's
/// record unchanged. Keeping the stored JSON line beside that record lets the
/// merged shard preserve the line's bytes, so content this build would encode
/// differently survives the write untouched. The writer normalizes line
/// terminators to `\n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRow {
    /// The row exactly as the input file held it, without its newline.
    line: String,
    /// The record parsed from that line.
    record: Value,
    /// The line number of the row in the input file, from 1.
    line_number: usize,
}

impl StoredRow {
    /// Returns the record parsed from this stored row.
    #[must_use]
    pub const fn record(&self) -> &Value {
        &self.record
    }
}

/// Reads the merge input of `side`, keeping each stored row beside its record.
///
/// An input is a version of the state file `shard_path`, so each line that is
/// not blank goes through the line checks that a read of that file makes:
/// those of its record type when [`ShardFamily::for_shard_path`] knows it,
/// else those of a line of no known record type. A refusal is
/// `invalid_state_file` at `shard_path` and the line of the input, and its
/// message starts with the side, as the refusal of [`ensure_unique_ids`] does.
pub fn read_jsonl_rows_for_shard(
    path: &camino::Utf8Path,
    shard_path: &camino::Utf8Path,
    side: MergeSide,
) -> anyhow::Result<Vec<StoredRow>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read(path)?;
    let read_line = ShardFamily::for_shard_path(shard_path).line_reader();
    let mut rows = Vec::new();
    for (line_number, bytes) in numbered_lines(&contents) {
        if std::str::from_utf8(bytes).is_ok_and(|line| line.trim().is_empty()) {
            continue;
        }
        let (line, record) = read_line(shard_path, line_number, bytes)
            .map_err(|error| with_side(error, shard_path, line_number, side))?;
        rows.push(StoredRow {
            line: line.to_owned(),
            record,
            line_number,
        });
    }
    Ok(rows)
}

/// The refusal `error` of line `line_number` with `side` at the start of its
/// message.
fn with_side(
    error: anyhow::Error,
    shard_path: &camino::Utf8Path,
    line_number: usize,
    side: MergeSide,
) -> anyhow::Error {
    let Some(fault) = UnparsableStateFile::find(error.as_ref()) else {
        return error;
    };
    crate::refusal::state_fault(
        &crate::state_store::message_ids::ShardLine::new(shard_path, line_number),
        format!("{side}: {}", fault.message()),
    )
}

/// Reads the merge input of `side` at `path`, a file of no known record type.
pub fn read_jsonl_rows(path: &camino::Utf8Path, side: MergeSide) -> anyhow::Result<Vec<StoredRow>> {
    read_jsonl_rows_for_shard(path, path, side)
}

/// Chooses the stored line each merged record keeps.
///
/// Every merged record is one side's record unchanged, so each one matches a
/// stored row by value exactly. A record keeps the line it was stored with:
/// ours when our side holds it, theirs when only theirs does. The merge is
/// refused when no stored row matches, because re-encoding that record would
/// publish content that no side stored.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
pub(crate) fn preserved_lines(
    ours: &[StoredRow],
    theirs: &[StoredRow],
    merged: &[CanonicalRecord],
) -> anyhow::Result<Vec<String>> {
    merged
        .iter()
        .map(|record| stored_row(ours, theirs, record).map(|(_, row)| row.line.clone()))
        .collect()
}

/// The stored row that holds a merged record, and its side: the row of ours
/// when ours holds the record, else the row of theirs. A record that no row
/// holds is refused.
fn stored_row<'a>(
    ours: &'a [StoredRow],
    theirs: &'a [StoredRow],
    record: &CanonicalRecord,
) -> anyhow::Result<(MergeSide, &'a StoredRow)> {
    let ours = ours.iter().map(|row| (MergeSide::Ours, row));
    let theirs = theirs.iter().map(|row| (MergeSide::Theirs, row));
    ours.chain(theirs)
        .find(|(_, row)| &row.record == record)
        .ok_or_else(|| {
            let named = record
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<record with no id>");
            crate::refusal::fault!(
                "merged record {named} matches no stored line; refusing to re-encode it"
            )
        })
}

/// Merges by the selected key, keeping a change when the other side matches the
/// base. Call [`validate_merged_records`] before publication: records valid alone
/// can be invalid together.
#[rule("rule_record_merge")]
pub fn merge_records(
    key: &str,
    base: &[CanonicalRecord],
    ours: &[CanonicalRecord],
    theirs: &[CanonicalRecord],
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    let base = index_by_id(key, base)?;
    let ours = index_by_id(key, ours)?;
    let theirs = index_by_id(key, theirs)?;
    let mut ids = BTreeSet::new();
    ids.extend(base.keys().cloned());
    ids.extend(ours.keys().cloned());
    ids.extend(theirs.keys().cloned());

    let mut merged = Vec::new();
    let mut conflicts = Vec::new();
    for id in ids {
        match (base.get(&id), ours.get(&id), theirs.get(&id)) {
            (None, Some(ours), None) => merged.push(ours.clone()),
            (None, None, Some(theirs)) => merged.push(theirs.clone()),
            (None, Some(ours), Some(theirs)) if ours == theirs => merged.push(ours.clone()),
            (Some(_), None, None) => {}
            (Some(base), Some(ours), None) if ours == base => {}
            (Some(base), Some(ours), None) => {
                merged.push(ours.clone());
                conflicts.push(MergeConflict {
                    kind: MergeConflictKind::DeleteModify,
                    record_id: id,
                    base: Some(base.clone()),
                    ours: Some(ours.clone()),
                    theirs: None,
                });
            }
            (Some(base), None, Some(theirs)) if theirs == base => {}
            (Some(base), None, Some(theirs)) => {
                merged.push(theirs.clone());
                conflicts.push(MergeConflict {
                    kind: MergeConflictKind::DeleteModify,
                    record_id: id,
                    base: Some(base.clone()),
                    ours: None,
                    theirs: Some(theirs.clone()),
                });
            }
            (Some(_), Some(ours), Some(theirs)) if ours == theirs => merged.push(ours.clone()),
            (Some(base), Some(ours), Some(theirs)) if ours == base => merged.push(theirs.clone()),
            (Some(base), Some(ours), Some(theirs)) if theirs == base => merged.push(ours.clone()),
            (base, Some(ours), Some(theirs)) => {
                merged.push(ours.clone());
                conflicts.push(MergeConflict {
                    kind: if base.is_none() {
                        MergeConflictKind::AddAdd
                    } else {
                        MergeConflictKind::DivergentEdit
                    },
                    record_id: id,
                    base: base.cloned(),
                    ours: Some(ours.clone()),
                    theirs: Some(theirs.clone()),
                });
            }
            (None, None, None) => unreachable!(),
        }
    }

    if conflicts.is_empty() {
        Ok(MergeOutcome::Clean { records: merged })
    } else {
        Ok(MergeOutcome::Conflicted {
            conflicts,
            partial: merged,
        })
    }
}

/// One side of a three-way merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeSide {
    Base,
    Ours,
    Theirs,
}

impl std::fmt::Display for MergeSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Base => "base",
            Self::Ours => "ours",
            Self::Theirs => "theirs",
        })
    }
}

/// Refuses one side of a merge that holds a record id on two rows.
///
/// Call it before [`merge_records`], on base, then ours, then theirs. The
/// rows are checked in their order, as [`merge_records`] checks them, so the
/// first fault of a side wins. A row with no string id gets the refusal of
/// [`merge_records`]. A second row with an id is `invalid_state_file` at
/// `shard_path` and its line, and the message names the side, the id and the
/// line of the first row.
pub fn ensure_unique_ids(
    shard_path: &camino::Utf8Path,
    side: MergeSide,
    key: &str,
    rows: &[StoredRow],
) -> anyhow::Result<()> {
    let mut first_lines = BTreeMap::new();
    for row in rows {
        let id = string_id(key, &row.record)?;
        if let Some(first) = first_lines.insert(id, row.line_number) {
            return Err(crate::refusal::state_fault(
                &crate::state_store::message_ids::ShardLine::new(shard_path, row.line_number),
                format!(
                    "{side} holds record id {id} at line {first} and at line {}",
                    row.line_number
                ),
            ));
        }
    }
    Ok(())
}

fn string_id<'a>(key: &str, record: &'a CanonicalRecord) -> anyhow::Result<&'a str> {
    record
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| crate::refusal::fault!("record is missing string {key}"))
}

fn index_by_id(
    key: &str,
    records: &[CanonicalRecord],
) -> anyhow::Result<BTreeMap<String, CanonicalRecord>> {
    let mut indexed = BTreeMap::new();
    for record in records {
        let id = string_id(key, record)?;
        crate::write_error::ensure!(Fault, !indexed.contains_key(id), "duplicate record id {id}");
        indexed.insert(id.to_string(), record.clone());
    }
    Ok(indexed)
}

#[cfg(test)]
mod preservation_tests;
#[cfg(test)]
mod tests;
