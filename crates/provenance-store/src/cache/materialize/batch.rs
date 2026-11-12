//! Multi-row inserts for the projection loaders.
//!
//! A loader adds its rows in the order it used to insert them one by one.
//! [`Rows::write`] sends them as multi-row statements, one table at a time.
//! The rows of one table keep their order, so each table gets the same row
//! ids as before. The statements run as one attempt of the transaction:
//! when one fails, none of them is kept, and [`Rows::write`] inserts the
//! rows one by one in the order the loader added them. So the first
//! failing row and its error are the same as with single-row inserts.

use crate::read_database::{SqlValue, Statement, Transaction};
use provenance_core::model::ColumnValue;
use std::sync::Arc;

/// One table that a loader writes. The names come from the projection
/// schema, never from input.
pub(super) struct Table {
    pub(super) name: String,
    pub(super) columns: Vec<String>,
    verb: &'static str,
}

impl Table {
    /// A plain insert into `name`.
    pub(super) fn insert(name: &str, columns: &[String]) -> Self {
        Self::with_verb("INSERT", name, columns)
    }

    /// The same insert, but a row equal to a stored row is skipped.
    pub(super) fn insert_or_ignore(name: &str, columns: &[String]) -> Self {
        Self::with_verb("INSERT OR IGNORE", name, columns)
    }

    fn with_verb(verb: &'static str, name: &str, columns: &[String]) -> Self {
        Self {
            name: name.to_owned(),
            columns: columns.to_vec(),
            verb,
        }
    }

    /// The same table with no rows, to write a part of a load.
    pub(super) fn again(&self) -> Self {
        Self::with_verb(self.verb, &self.name, &self.columns)
    }

    fn statement(&self, rows: usize) -> Arc<str> {
        let row = format!("({})", vec!["?"; self.columns.len()].join(", "));
        format!(
            "{} INTO {} ({}) VALUES {}",
            self.verb,
            self.name,
            self.columns.join(", "),
            vec![row; rows].join(", ")
        )
        .into()
    }
}

/// The rows of one load, for one or more tables, in the loader's order.
/// They come with the number of records they hold, and with the error that
/// stopped the loader, if one did.
pub(super) struct Rows {
    pub(super) tables: Vec<Table>,
    pub(super) entries: Vec<(usize, Vec<ColumnValue>)>,
    pub(super) loaded: u64,
    pub(super) failure: Option<anyhow::Error>,
}

impl Rows {
    pub(super) const fn new(tables: Vec<Table>) -> Self {
        Self {
            tables,
            entries: Vec::new(),
            loaded: 0,
            failure: None,
        }
    }

    /// Adds one row for the table at `table` in the list given to [`Rows::new`].
    pub(super) fn push(&mut self, table: usize, row: Vec<ColumnValue>) {
        debug_assert_eq!(row.len(), self.tables[table].columns.len());
        self.entries.push((table, row));
    }

    /// Writes every row, then reports the number of records or the error
    /// that stopped the loader. A statement binds at most `bound_values`
    /// values. See the module comment for the order on failure.
    ///
    /// The values of the rows move into the statements, and back out of
    /// them for the one-by-one inserts, so the load holds one copy of them.
    pub(super) async fn write(
        self,
        tx: &mut Transaction,
        bound_values: usize,
    ) -> anyhow::Result<u64> {
        let Self {
            tables,
            entries,
            loaded,
            failure,
        } = self;
        if !entries.is_empty() {
            let order: Vec<usize> = entries.iter().map(|(table, _)| *table).collect();
            let (of, statements) = batched(&tables, entries, bound_values);
            if tx.attempt(&statements).await?.is_err() {
                one_by_one(tx, &tables, &order, &of, statements).await?;
            }
        }
        failure.map_or(Ok(loaded), Err)
    }
}

/// The multi-row statements of `entries`, one table at a time, with the
/// table of each statement.
fn batched(
    tables: &[Table],
    entries: Vec<(usize, Vec<ColumnValue>)>,
    bound_values: usize,
) -> (Vec<usize>, Vec<Statement>) {
    let mut rows: Vec<Vec<Vec<ColumnValue>>> = tables.iter().map(|_| Vec::new()).collect();
    for (table, row) in entries {
        rows[table].push(row);
    }
    let (mut of, mut statements) = (Vec::new(), Vec::new());
    for (index, (table, rows)) in tables.iter().zip(rows).enumerate() {
        let per_statement = (bound_values / table.columns.len().max(1)).max(1);
        let full = table.statement(per_statement);
        let mut rows = rows.into_iter().peekable();
        while rows.peek().is_some() {
            let chunk: Vec<Vec<ColumnValue>> = rows.by_ref().take(per_statement).collect();
            let sql = if chunk.len() == per_statement {
                Arc::clone(&full)
            } else {
                table.statement(chunk.len())
            };
            of.push(index);
            statements.push(Statement {
                sql: sql.to_string(),
                values: chunk.into_iter().flatten().map(into_value).collect(),
            });
        }
    }
    (of, statements)
}

/// Inserts the rows of `statements` one by one, in the loader's `order`.
/// `of` names the table of each statement.
async fn one_by_one(
    tx: &mut Transaction,
    tables: &[Table],
    order: &[usize],
    of: &[usize],
    statements: Vec<Statement>,
) -> anyhow::Result<()> {
    let mut values: Vec<Vec<SqlValue>> = tables.iter().map(|_| Vec::new()).collect();
    for (table, statement) in of.iter().zip(statements) {
        values[*table].extend(statement.values);
    }
    let mut values: Vec<_> = values.into_iter().map(Vec::into_iter).collect();
    let single: Vec<Arc<str>> = tables.iter().map(|table| table.statement(1)).collect();
    for table in order {
        let row = values[*table]
            .by_ref()
            .take(tables[*table].columns.len())
            .collect();
        tx.execute(&single[*table], row).await?;
    }
    Ok(())
}

/// The value that a statement binds for one column value.
fn into_value(column: ColumnValue) -> SqlValue {
    match column {
        ColumnValue::Null => SqlValue::Null,
        ColumnValue::Integer(integer) => SqlValue::Integer(integer),
        ColumnValue::Real(real) => SqlValue::Real(real),
        ColumnValue::Text(text) => SqlValue::Text(text),
    }
}

/// A text value.
pub(super) fn text(value: impl Into<String>) -> ColumnValue {
    ColumnValue::Text(value.into())
}

/// A text value, or null when absent.
pub(super) fn optional_text(value: Option<impl Into<String>>) -> ColumnValue {
    value.map_or(ColumnValue::Null, text)
}

#[cfg(test)]
#[cfg(feature = "host")]
mod tests;
