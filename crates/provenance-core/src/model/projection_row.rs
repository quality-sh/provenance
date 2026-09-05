//! One projection table row per record: one column for each field.
//!
//! A record kind implements [`ProjectionRow`] through
//! `#[derive(ProjectionRow)]`. The derive names the table, lists the fields
//! as columns in declaration order, and writes `row` and `from_row` over
//! the helpers below. Values pass through JSON: a string is text, a number
//! is an integer or a real, a bool is 0 or 1, an absent value is null, and
//! a list or a struct is text that holds its JSON. Core has no database
//! dependency; the store binds and reads the values.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Number, Value};

/// One column value, in the four storage classes a projection column can
/// hold.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

/// The result type the derive writes into its signatures.
pub type RowResult<T> = anyhow::Result<T>;

/// A record kind stored as one table with one column per field.
pub trait ProjectionRow: Serialize + DeserializeOwned + Sized {
    /// The table name.
    const TABLE: &'static str;

    /// The field names, in declaration order.
    const COLUMNS: &'static [&'static str];

    /// The record's values, one per column, in `COLUMNS` order.
    fn row(&self) -> RowResult<Vec<ColumnValue>>;

    /// The record read back from one row in `COLUMNS` order.
    fn from_row(row: &[ColumnValue]) -> RowResult<Self>;
}

/// Encodes one field. A list or a struct becomes the text of its JSON.
pub fn encode<T: Serialize>(field: &T) -> RowResult<ColumnValue> {
    Ok(match serde_json::to_value(field)? {
        Value::Null => ColumnValue::Null,
        Value::Bool(flag) => ColumnValue::Integer(i64::from(flag)),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                ColumnValue::Integer(integer)
            } else if number.is_f64() {
                ColumnValue::Real(number.as_f64().expect("an f64 number"))
            } else {
                anyhow::bail!("number {number} does not fit a column")
            }
        }
        Value::String(text) => ColumnValue::Text(text),
        composite @ (Value::Array(_) | Value::Object(_)) => {
            ColumnValue::Text(composite.to_string())
        }
    })
}

/// A column read as the JSON scalar it holds.
pub fn scalar(value: &ColumnValue) -> RowResult<Value> {
    Ok(match value {
        ColumnValue::Null => Value::Null,
        ColumnValue::Integer(integer) => Value::Number((*integer).into()),
        ColumnValue::Real(real) => Number::from_f64(*real)
            .map(Value::Number)
            .ok_or_else(|| anyhow::anyhow!("real {real} is not a JSON number"))?,
        ColumnValue::Text(text) => Value::String(text.clone()),
    })
}

/// A column holding JSON text; null reads as JSON null.
pub fn json(value: &ColumnValue) -> RowResult<Value> {
    match value {
        ColumnValue::Null => Ok(Value::Null),
        ColumnValue::Text(text) => Ok(serde_json::from_str(text)?),
        other => anyhow::bail!("a JSON column holds text, not {other:?}"),
    }
}

/// A column holding 0 or 1; null reads as JSON null.
pub fn flag(value: &ColumnValue) -> RowResult<Value> {
    match value {
        ColumnValue::Null => Ok(Value::Null),
        ColumnValue::Integer(0) => Ok(Value::Bool(false)),
        ColumnValue::Integer(1) => Ok(Value::Bool(true)),
        other => anyhow::bail!("a bool column holds 0 or 1, not {other:?}"),
    }
}

/// Checks a row has one value per column.
pub fn columns<K: ProjectionRow>(row: &[ColumnValue]) -> RowResult<&[ColumnValue]> {
    anyhow::ensure!(
        row.len() == K::COLUMNS.len(),
        "{} has {} columns; the row holds {}",
        K::TABLE,
        K::COLUMNS.len(),
        row.len()
    );
    Ok(row)
}

/// Builds the record from its decoded fields.
pub fn record<K: ProjectionRow>(fields: Vec<(&'static str, Value)>) -> RowResult<K> {
    let object: Map<String, Value> = fields
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
        .collect();
    Ok(serde_json::from_value(Value::Object(object))?)
}
