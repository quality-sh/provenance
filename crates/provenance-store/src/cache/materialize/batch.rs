//! Multi-row inserts for the projection loaders.
//!
//! A loader adds its rows in the order it used to insert them one by one.
//! [`Rows::write`] sends them as multi-row statements, one table at a time.
//! The rows of one table keep their order, so each table gets the same row
//! ids as before. When a statement fails, [`Rows::write`] rolls the batch
//! back to its savepoint. It then inserts the rows one by one in the order
//! the loader added them. So the first failing row and its error are the
//! same as with single-row inserts.

use provenance_core::model::ColumnValue;
use sqlx::sqlite::SqliteArguments;
use sqlx::{query::Query, Sqlite, Transaction};
use std::sync::Arc;

/// The most values that one statement binds.
const VALUES_PER_STATEMENT: usize = 999;

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
    /// that stopped the loader. See the module comment for the order on
    /// failure.
    pub(super) async fn write(self, tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<u64> {
        if !self.entries.is_empty() {
            execute(tx, "SAVEPOINT projection_rows").await?;
            if self.batched(tx).await.is_ok() {
                execute(tx, "RELEASE projection_rows").await?;
            } else {
                execute(tx, "ROLLBACK TO projection_rows").await?;
                execute(tx, "RELEASE projection_rows").await?;
                self.one_by_one(tx).await?;
            }
        }
        self.failure.map_or(Ok(self.loaded), Err)
    }

    async fn one_by_one(&self, tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
        for (table, row) in &self.entries {
            let mut query = sqlx::query(sqlx::AssertSqlSafe(self.tables[*table].statement(1)));
            for value in row {
                query = bind(query, value);
            }
            query.execute(&mut **tx).await?;
        }
        Ok(())
    }

    async fn batched(&self, tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
        for (index, table) in self.tables.iter().enumerate() {
            let rows: Vec<&Vec<ColumnValue>> = self
                .entries
                .iter()
                .filter(|(of, _)| *of == index)
                .map(|(_, row)| row)
                .collect();
            let per_statement = (VALUES_PER_STATEMENT / table.columns.len().max(1)).max(1);
            let full = table.statement(per_statement);
            for chunk in rows.chunks(per_statement) {
                let sql = if chunk.len() == per_statement {
                    Arc::clone(&full)
                } else {
                    table.statement(chunk.len())
                };
                let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
                for value in chunk.iter().flat_map(|row| row.iter()) {
                    query = bind(query, value);
                }
                query.execute(&mut **tx).await?;
            }
        }
        Ok(())
    }
}

async fn execute(tx: &mut Transaction<'_, Sqlite>, sql: &'static str) -> anyhow::Result<()> {
    sqlx::query(sql).execute(&mut **tx).await?;
    Ok(())
}

pub(super) fn bind<'q>(
    query: Query<'q, Sqlite, SqliteArguments>,
    value: &ColumnValue,
) -> Query<'q, Sqlite, SqliteArguments> {
    match value {
        ColumnValue::Null => query.bind(None::<String>),
        ColumnValue::Integer(integer) => query.bind(*integer),
        ColumnValue::Real(real) => query.bind(*real),
        ColumnValue::Text(text) => query.bind(text.clone()),
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
