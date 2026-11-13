//! One `SQLite` row into one record.
//!
//! The select lists `K::COLUMNS` in order, so column `i` is field `i`.
//! Each value keeps the storage class that the read database gives for
//! it: a `REAL` column holding `1.0` must stay a real, or a confidence of
//! one would print as `1`.

use crate::cache::quoted;
use crate::read_database::{SqlRow, SqlValue};
use provenance_core::model::{ColumnValue, ProjectionRow};

/// The quoted column list of one record type, for a select.
pub fn select_columns<K: ProjectionRow>() -> String {
    K::COLUMNS
        .iter()
        .map(|column| quoted(column))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The record a row holds, read column by column. The values move out of
/// the row, so no text is copied.
pub(super) fn decode<K: ProjectionRow>(row: SqlRow) -> anyhow::Result<K> {
    crate::test_probes::at("query_row_hydrated")?;
    K::from_row(&column_values::<K>(row)?)
}

/// The row's values in `COLUMNS` order, each in the storage class the
/// database reports. The first column that no field takes, in column
/// order, refuses the row.
pub fn column_values<K: ProjectionRow>(row: SqlRow) -> anyhow::Result<Vec<ColumnValue>> {
    let wanted = K::COLUMNS.len();
    let len = row.len();
    let mut values = Vec::with_capacity(wanted);
    for (index, value) in row.into_iter().take(wanted).enumerate() {
        values.push(column_value(value, index)?);
    }
    if len < wanted {
        return Err(crate::refusal::fault!(
            "column index out of bounds: the len is {len}, but the index is {len}"
        ));
    }
    Ok(values)
}

/// The index of a record type's `id` column in its select list.
pub(super) fn id_column<K: ProjectionRow>() -> anyhow::Result<usize> {
    K::COLUMNS
        .iter()
        .position(|column| *column == "id")
        .ok_or_else(|| crate::refusal::fault!("no column found for name: id"))
}

fn column_value(value: SqlValue, index: usize) -> anyhow::Result<ColumnValue> {
    Ok(match value {
        SqlValue::Null => ColumnValue::Null,
        SqlValue::Integer(value) => ColumnValue::Integer(value),
        SqlValue::Real(value) => ColumnValue::Real(value),
        SqlValue::Text(value) => ColumnValue::Text(value),
        SqlValue::Blob(_) => {
            return Err(crate::refusal::fault!(
                "column {index} holds a BLOB value, which no record field takes"
            ))
        }
    })
}
