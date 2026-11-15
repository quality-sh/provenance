//! A read database in one `SQLite` connection in memory, outside the store.
//!
//! The store has no read database of its own without `host`. A test that
//! runs the read operations without `host` uses this one, as an embedder
//! uses its own: through the port only. The connection lives as long as the
//! adapter, so the projection stays between calls.
//!
//! It meets the read database contract of the store as the file adapter
//! does: `read_operations_on_memory/adapter.rs` runs the contract cases on
//! it. A read before the first write finds no database; a bound value or a
//! row past the integer range of the port is refused; and the work bound
//! stops a statement with the `SQLite` progress handler.

use provenance_store::read_database::{
    OutOfRange, OverBudget, ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue,
    Statement, Transaction, Unopened, WriteWork,
};
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool, TypeInfo, ValueRef};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The engine steps between two calls of the progress handler that bounds
/// the work of a transaction, as in the file adapter.
const STEPS_PER_CALL: u64 = 1000;

/// One `SQLite` database in memory. A clone is the same database, as a
/// later process opens it.
#[derive(Debug, Clone)]
pub struct MemorySqlite {
    pool: SqlitePool,
    /// Whether a write has made the database. Before it, a read finds no
    /// database, as a read of the file adapter finds no file.
    made: Arc<AtomicBool>,
}

impl MemorySqlite {
    /// A database that nothing has written. It must be made on a Tokio
    /// runtime.
    pub async fn new() -> Self {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await
            .unwrap();
        Self {
            pool,
            made: Arc::default(),
        }
    }

    /// Begins one transaction on the one connection, with no work bound: a
    /// bound that an earlier transaction set stays on the connection until
    /// it is removed.
    async fn begin(&self) -> anyhow::Result<Transaction> {
        let mut transaction = self.pool.begin().await?;
        transaction.lock_handle().await?.remove_progress_handler();
        Ok(Box::new(MemoryTransaction(transaction)))
    }

    /// Begins a read, or refuses before the work when no write made the
    /// database.
    async fn begin_read(&self) -> anyhow::Result<Transaction> {
        if !self.made.load(Ordering::SeqCst) {
            return Err(Unopened(anyhow::anyhow!(
                "the read database in memory holds nothing: no write made it"
            ))
            .into());
        }
        self.begin().await
    }

    /// Begins a write, which makes the database.
    async fn begin_write(&self) -> anyhow::Result<Transaction> {
        self.made.store(true, Ordering::SeqCst);
        self.begin().await
    }
}

impl ReadDatabase for MemorySqlite {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move { work(self.begin_read().await?).await })
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move { work(self.begin_write().await?).await })
    }

    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            write(self.begin_write().await?).await?;
            Ok(async { read(self.begin_read().await?).await }.await)
        })
    }

    fn max_bound_values(&self) -> usize {
        999
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        Ok([7; 32])
    }

    fn location(&self) -> String {
        "memory".to_owned()
    }
}

struct MemoryTransaction(sqlx::Transaction<'static, Sqlite>);

impl SqlTransaction for MemoryTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let rows = bound(sql, values)?
                .fetch_all(&mut *self.0)
                .await
                .map_err(statement_error)?;
            rows.iter().map(decode).collect()
        })
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        Box::pin(async move {
            Ok(bound(sql, values)?
                .execute(&mut *self.0)
                .await
                .map_err(statement_error)?
                .rows_affected())
        })
    }

    /// A statement that writes and that the work bound stops ends the
    /// transaction and its savepoint, so then the bound is the failure.
    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>> {
        Box::pin(async move {
            self.execute("SAVEPOINT attempt", Vec::new()).await?;
            let mut failure = None;
            for statement in statements {
                if let Err(error) = self.execute(&statement.sql, statement.values.clone()).await {
                    failure = Some(error);
                    break;
                }
            }
            if let Some(error) = failure {
                if let Err(rollback) = self.execute("ROLLBACK TO attempt", Vec::new()).await {
                    return Err(if error.is::<OverBudget>() {
                        error
                    } else {
                        rollback
                    });
                }
                self.execute("RELEASE attempt", Vec::new()).await?;
                return Ok(Err(error));
            }
            self.execute("RELEASE attempt", Vec::new()).await?;
            Ok(Ok(()))
        })
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        Box::pin(async move { Ok(self.0.commit().await?) })
    }

    /// Sets the `SQLite` progress handler on the connection, as the file
    /// adapter does: one call each [`STEPS_PER_CALL`] steps, and a handler
    /// that answers false stops the statement with `SQLITE_INTERRUPT`.
    fn bound_work(&mut self, bound: u64) -> SqlFuture<'_, ()> {
        Box::pin(async move {
            let most = bound / STEPS_PER_CALL;
            let mut handle = self.0.lock_handle().await?;
            let mut calls = 0;
            handle.set_progress_handler(i32::try_from(STEPS_PER_CALL)?, move || {
                calls += 1;
                calls <= most
            });
            Ok(())
        })
    }

    fn max_list_values(&self) -> usize {
        500
    }
}

/// The statement `sql` with `values` bound. An integer past the range of
/// the port is refused before the statement runs.
fn bound(
    sql: &str,
    values: Vec<SqlValue>,
) -> Result<Query<'_, Sqlite, SqliteArguments>, OutOfRange> {
    OutOfRange::check(&values)?;
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for value in values {
        query = match value {
            SqlValue::Null => query.bind(None::<i64>),
            SqlValue::Integer(value) => query.bind(value),
            SqlValue::Real(value) => query.bind(value),
            SqlValue::Text(value) => query.bind(value),
            SqlValue::Blob(value) => query.bind(value),
        };
    }
    Ok(query)
}

/// A statement that the progress handler stopped, `SQLite` code 9
/// (`SQLITE_INTERRUPT`), is [`OverBudget`].
fn statement_error(error: sqlx::Error) -> anyhow::Error {
    let interrupted = error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "9");
    if interrupted {
        OverBudget(error.into()).into()
    } else {
        error.into()
    }
}

fn decode(row: &SqliteRow) -> anyhow::Result<SqlRow> {
    (0..row.len())
        .map(|index| -> anyhow::Result<SqlValue> {
            let raw = row.try_get_raw(index)?;
            if raw.is_null() {
                return Ok(SqlValue::Null);
            }
            Ok(match raw.type_info().name() {
                "REAL" => SqlValue::Real(row.try_get(index)?),
                "TEXT" => SqlValue::Text(row.try_get(index)?),
                "BLOB" => SqlValue::Blob(row.try_get(index)?),
                _ => SqlValue::Integer(OutOfRange::integer(row.try_get(index)?)?),
            })
        })
        .collect()
}
