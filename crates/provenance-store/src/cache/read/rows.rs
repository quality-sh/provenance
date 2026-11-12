//! One `SQLite` row into one record.
//!
//! The select lists `K::COLUMNS` in order, so column `i` is field `i`.
//! Each value keeps the storage class that the read database gives for
//! it: a `REAL` column holding `1.0` must stay a real, or a confidence of
//! one would print as `1`.

use crate::cache::quoted;
use crate::read_database::SqlValue;
use provenance_core::model::{ColumnValue, ProjectionRow};

/// The quoted column list of one record type, for a select.
pub fn select_columns<K: ProjectionRow>() -> String {
    K::COLUMNS
        .iter()
        .map(|column| quoted(column))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The record a row holds, read column by column.
pub(super) fn decode<K: ProjectionRow>(row: &[SqlValue]) -> anyhow::Result<K> {
    crate::test_probes::at("query_row_hydrated")?;
    K::from_row(&column_values::<K>(row)?)
}

/// The row's values in `COLUMNS` order, each in the storage class the
/// database reports.
pub fn column_values<K: ProjectionRow>(row: &[SqlValue]) -> anyhow::Result<Vec<ColumnValue>> {
    (0..K::COLUMNS.len())
        .map(|index| column_value(row, index))
        .collect()
}

/// The index of a record type's `id` column in its select list.
pub(super) fn id_column<K: ProjectionRow>() -> anyhow::Result<usize> {
    K::COLUMNS
        .iter()
        .position(|column| *column == "id")
        .ok_or_else(|| crate::refusal::fault!("no column found for name: id"))
}

fn column_value(row: &[SqlValue], index: usize) -> anyhow::Result<ColumnValue> {
    Ok(match row.get(index) {
        Some(SqlValue::Null) => ColumnValue::Null,
        Some(SqlValue::Integer(value)) => ColumnValue::Integer(*value),
        Some(SqlValue::Real(value)) => ColumnValue::Real(*value),
        Some(SqlValue::Text(value)) => ColumnValue::Text(value.clone()),
        Some(SqlValue::Blob(_)) => {
            return Err(crate::refusal::fault!(
                "column {index} holds a BLOB value, which no record field takes"
            ))
        }
        None => {
            return Err(crate::refusal::fault!(
                "column index out of bounds: the len is {}, but the index is {index}",
                row.len()
            ))
        }
    })
}
