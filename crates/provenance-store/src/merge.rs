use batches::RecordConflicts;
use crate::state_store::readers::ensure_no_legacy_retirement;
use provenance_macros::rule;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod batches;
pub mod validation;

pub use batches::BatchRecords;
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

/// Reads one JSONL merge input, keeping each stored row beside its record.
pub fn read_jsonl_rows_for_shard(
    path: &camino::Utf8Path,
    shard_path: &camino::Utf8Path,
) -> anyhow::Result<Vec<StoredRow>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut rows = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str(line)?;
        // Errors name the logical shard, but line numbers come from Git's temporary input file.
        ensure_no_legacy_retirement(shard_path, index + 1, &record)?;
        rows.push(StoredRow {
            line: line.to_owned(),
            record,
            line_number: index + 1,
        });
    }
    Ok(rows)
}

/// Reads one JSONL merge input with no shard type known.
pub fn read_jsonl_rows(path: &camino::Utf8Path) -> anyhow::Result<Vec<StoredRow>> {
    read_jsonl_rows_for_shard(path, path)
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
        .map(|record| {
            let named = record
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<record with no id>");
            ours.iter()
                .chain(theirs.iter())
                .find(|row| &row.record == record)
                .map(|row| row.line.clone())
                .ok_or_else(|| {
                    crate::refusal::fault!(
                        "merged record {named} matches no stored line; refusing to re-encode it"
                    )
                })
        })
        .collect()
}

/// What makes rows of the sides of a merge the same record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIdentity {
    /// Rows with the same string in this field are one record. The merge
    /// writes the records in the order of that string.
    Field(&'static str),
    /// Equal rows are the same batch. One side can hold equal batches, so the
    /// second equal row of one side is the same batch as the second equal row
    /// of another side. The merge writes the batches of the base, then the
    /// batches that ours adds, then the batches that theirs adds, each in the
    /// order of its file. A batch holds the records that [`BatchRecords`]
    /// names, and the merge compares them as records.
    Batch(BatchRecords),
}

impl RowIdentity {
    /// The merge order of the keys of the three sides.
    fn merge_order(self, sides: [&[(String, CanonicalRecord)]; 3]) -> Vec<String> {
        let keys = sides
            .into_iter()
            .flat_map(|side| side.iter().map(|(key, _)| key.clone()));
        match self {
            Self::Field(_) => keys.collect::<BTreeSet<_>>().into_iter().collect(),
            Self::Batch(_) => {
                let mut seen = BTreeSet::new();
                keys.filter(|key| seen.insert(key.clone())).collect()
            }
        }
    }

    /// The conflicts between the records that the rows of the sides hold.
    fn record_conflicts(
        self,
        sides: [&[(String, CanonicalRecord)]; 3],
    ) -> anyhow::Result<batches::RecordConflicts> {
        match self {
            Self::Field(_) => Ok(batches::RecordConflicts::default()),
            Self::Batch(records) => batches::record_conflicts(records, sides),
        }
    }
}

/// Gives the key of each row of one side, in the order of the rows.
struct RowKeys {
    identity: RowIdentity,
    /// For a batch identity, the number of rows so far equal to each row.
    equal_rows: BTreeMap<String, usize>,
}

impl RowKeys {
    const fn new(identity: RowIdentity) -> Self {
        Self {
            identity,
            equal_rows: BTreeMap::new(),
        }
    }

    fn key(&mut self, record: &CanonicalRecord) -> anyhow::Result<String> {
        match self.identity {
            RowIdentity::Field(field) => string_id(field, record).map(str::to_owned),
            RowIdentity::Batch(_) => batch_key(&mut self.equal_rows, record),
        }
    }
}

/// The key of one batch row: its canonical JSON and the number of rows of
/// its side up to it that are equal to it.
#[rule("rule_landing_batch_merge")]
fn batch_key(
    equal_rows: &mut BTreeMap<String, usize>,
    record: &CanonicalRecord,
) -> anyhow::Result<String> {
    let row = String::from_utf8(crate::canonical_digest::canonical_bytes(record)?)?;
    let count = equal_rows.entry(row.clone()).or_default();
    *count += 1;
    Ok(format!("{row}#{count}"))
}

/// Merges by the row identity of the shard, keeping a change when the other
/// side matches the base; the records in batch rows merge as records too.
/// Call [`validate_merged_records`] before publication.
#[rule("rule_record_merge")]
pub fn merge_records(
    identity: RowIdentity,
    base: &[CanonicalRecord],
    ours: &[CanonicalRecord],
    theirs: &[CanonicalRecord],
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    let base = keyed_rows(identity, base)?;
    let ours = keyed_rows(identity, ours)?;
    let theirs = keyed_rows(identity, theirs)?;
    let RecordConflicts {
        conflicts: record_conflicts,
        left_out,
    } = identity.record_conflicts([&base, &ours, &theirs])?;
    let mut ids = identity.merge_order([&base, &ours, &theirs]);
    ids.retain(|id| !left_out.contains(id));
    let [base, ours, theirs] =
        [base, ours, theirs].map(|rows| rows.into_iter().collect::<BTreeMap<_, _>>());

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
    conflicts.extend(record_conflicts);

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

/// Refuses one side of a merge that holds a record key on two rows.
///
/// Call it before [`merge_records`], on base, then ours, then theirs. The
/// rows are checked in their order, as [`merge_records`] checks them, so the
/// first fault of a side wins. For a field identity, a row with no string in
/// the field gets the refusal of [`merge_records`], and a second row with a
/// key is `invalid_state_file` at `shard_path` and its line: the message
/// names the side, the key and the line of the first row. For a batch
/// identity, the key of a row holds its count among the equal rows of its
/// side, so no two rows have one key and the function refuses only a row
/// that is not canonical JSON.
pub fn ensure_unique_ids(
    shard_path: &camino::Utf8Path,
    side: MergeSide,
    identity: RowIdentity,
    rows: &[StoredRow],
) -> anyhow::Result<()> {
    let mut keys = RowKeys::new(identity);
    let mut first_lines = BTreeMap::new();
    for row in rows {
        let id = keys.key(&row.record)?;
        if let Some(first) = first_lines.insert(id.clone(), row.line_number) {
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

pub fn read_jsonl_records(path: &camino::Utf8Path) -> anyhow::Result<Vec<CanonicalRecord>> {
    Ok(read_jsonl_rows(path)?
        .into_iter()
        .map(|row| row.record)
        .collect())
}

pub fn read_jsonl_records_for_shard(
    path: &camino::Utf8Path,
    shard_path: &camino::Utf8Path,
) -> anyhow::Result<Vec<CanonicalRecord>> {
    Ok(read_jsonl_rows_for_shard(path, shard_path)?
        .into_iter()
        .map(|row| row.record)
        .collect())
}

/// Each record of one side with its key, in the order of the side. A key on
/// two rows is refused at the second row.
fn keyed_rows(
    identity: RowIdentity,
    records: &[CanonicalRecord],
) -> anyhow::Result<Vec<(String, CanonicalRecord)>> {
    let mut keys = RowKeys::new(identity);
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for record in records {
        let id = keys.key(record)?;
        crate::write_error::ensure!(Fault, seen.insert(id.clone()), "duplicate record id {id}");
        rows.push((id, record.clone()));
    }
    Ok(rows)
}

#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod preservation_tests;
#[cfg(test)]
mod tests;
