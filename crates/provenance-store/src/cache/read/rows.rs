//! One `SQLite` row into one record.
//!
//! The select lists `K::COLUMNS` in order, so column `i` is field `i`.
//! Each value is read by its storage class through `type_info()`, never by
//! trying `i64` first: a `REAL` column holding `1.0` must stay a real, or
//! a confidence of one would print as `1`.

use crate::cache::quoted;
use provenance_core::model::{ColumnValue, ProjectionRow};
use sqlx::sqlite::{SqliteRow, SqliteValueRef};
use sqlx::{Decode, Row, TypeInfo, ValueRef};

/// The quoted column list of one record type, for a select.
pub(super) fn select_columns<K: ProjectionRow>() -> String {
    K::COLUMNS
        .iter()
        .map(|column| quoted(column))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The record a row holds, read column by column.
pub(super) fn decode<K: ProjectionRow>(row: &SqliteRow) -> anyhow::Result<K> {
    let values = (0..K::COLUMNS.len())
        .map(|index| column_value(row, index))
        .collect::<anyhow::Result<Vec<_>>>()?;
    K::from_row(&values)
}

fn column_value(row: &SqliteRow, index: usize) -> anyhow::Result<ColumnValue> {
    let raw = row.try_get_raw(index)?;
    if raw.is_null() {
        return Ok(ColumnValue::Null);
    }
    let class = raw.type_info().name().to_string();
    Ok(match class.as_str() {
        "INTEGER" => ColumnValue::Integer(read::<i64>(raw)?),
        "REAL" => ColumnValue::Real(read::<f64>(raw)?),
        "TEXT" => ColumnValue::Text(read::<String>(raw)?),
        other => anyhow::bail!("column {index} holds a {other} value, which no record field takes"),
    })
}

fn read<'r, T: Decode<'r, sqlx::Sqlite>>(raw: SqliteValueRef<'r>) -> anyhow::Result<T> {
    T::decode(raw).map_err(|error| anyhow::anyhow!(error))
}
