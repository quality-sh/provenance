//! The file adapter: the CLI's `SQLite` file under `.provenance/cache`.
//!
//! Each call opens the file once, runs its work (a write and then a read,
//! for `write_then_read`), and completes the connection before it gives
//! its result, as [`CacheConnection`] does. A read opens the stored file
//! and does not make it. A write makes the file in WAL mode.

use super::{
    OverBudget, ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement,
    Transaction, Unopened, WorkUnit, WriteWork,
};
use crate::cache::{open_cache, open_stored_cache, CacheConnection};
use crate::layout::ProvenanceLayout;
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool, TypeInfo, ValueRef};
use std::io::{Read, Write};

/// The maximum number of values that one statement binds in the file.
pub const MAX_BOUND_VALUES: usize = 999;

/// The most values of one list that a reader binds in one statement of
/// the file.
pub const MAX_LIST_VALUES: usize = 500;

/// The engine steps between two calls of the progress handler that bounds
/// the work of a transaction.
const STEPS_PER_CALL: u64 = 1000;

/// The read database in the cache directory of one layout.
#[derive(Debug, Clone)]
pub struct FileReadDatabase {
    layout: ProvenanceLayout,
}

impl FileReadDatabase {
    pub const fn new(layout: ProvenanceLayout) -> Self {
        Self { layout }
    }
}

impl ReadDatabase for FileReadDatabase {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            let connection = open_stored_cache(&self.layout).await.map_err(Unopened)?;
            let outcome = async {
                let transaction = begin(connection.pool()).await?;
                work(transaction).await
            }
            .await;
            connection.settle(outcome).await
        })
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            let connection = open_cache(&self.layout).await?;
            let outcome = run_write(&connection, work).await;
            connection.settle(outcome).await
        })
    }

    /// The read runs on the connection of the write, as the read after a
    /// catch-up did on that connection before the port. The connection
    /// completes once, after the read, or after the write when the write
    /// fails.
    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            let connection = open_cache(&self.layout).await?;
            if let Err(failure) = run_write(&connection, write).await {
                return Err(connection.close_reporting(failure).await);
            }
            let outcome = async { read(begin(connection.pool()).await?).await }.await;
            Ok(connection.settle(outcome).await)
        })
    }

    fn max_bound_values(&self) -> usize {
        MAX_BOUND_VALUES
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        let dir = self.layout.cache_dir();
        let path = dir.join("read-cursor.key");
        if !path.exists() {
            let mut file = tempfile::NamedTempFile::new_in(crate::long_path::extended(&dir)?)?;
            file.write_all(uuid::Uuid::new_v4().as_bytes())?;
            file.write_all(uuid::Uuid::new_v4().as_bytes())?;
            match file.persist_noclobber(crate::long_path::extended(&path)?) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(33)
            .read_to_end(&mut bytes)?;
        let key: Result<[u8; 32], _> = bytes.try_into();
        crate::write_error::ensure!(Fault, key.is_ok(), "invalid cursor signing key");
        Ok(key.unwrap_or_default())
    }

    fn location(&self) -> String {
        self.layout.cache_db_path().to_string()
    }
}

async fn run_write(connection: &CacheConnection, work: WriteWork<'_>) -> anyhow::Result<()> {
    let transaction = begin(connection.pool()).await?;
    work(transaction).await
}

/// Begins one transaction on an open pool of the file.
pub(crate) async fn begin(pool: &SqlitePool) -> anyhow::Result<Transaction> {
    crate::test_probes::at("read_database_begin")?;
    Ok(Box::new(FileTransaction(pool.begin().await?)))
}

struct FileTransaction(sqlx::Transaction<'static, Sqlite>);

impl SqlTransaction for FileTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let rows = bound(sql, values)
                .fetch_all(&mut *self.0)
                .await
                .map_err(statement_error)?;
            // Each row of the file is freed once it is decoded, so the
            // two forms of the rows are not held in full at once.
            rows.into_iter().map(|row| decode(&row)).collect()
        })
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        Box::pin(async move {
            Ok(bound(sql, values)
                .execute(&mut *self.0)
                .await
                .map_err(statement_error)?
                .rows_affected())
        })
    }

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
                    // A statement that writes and that the work bound
                    // stops ends the transaction, and its savepoint too.
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

    /// Sets the `SQLite` progress handler on the connection: one call each
    /// [`STEPS_PER_CALL`] steps of a statement, and a handler that answers
    /// false stops the statement while it runs, with `SQLITE_INTERRUPT`.
    /// The calls of all the statements count against one bound.
    ///
    /// `SQLite` counts the steps of one prepared statement over all its
    /// runs, and the connection keeps a prepared statement for each
    /// statement text. So a statement text whose runs together do fewer
    /// steps than one interval does not call the handler, and the steps of
    /// a text after its last call are not counted. A text that runs again
    /// adds its steps to the steps of its earlier runs.
    ///
    /// `SQLite` rolls back the whole transaction when it stops a statement
    /// that writes, and the connection then runs each later statement
    /// outside a transaction.
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

    fn work_unit(&self) -> WorkUnit {
        WorkUnit::EngineSteps
    }

    fn max_list_values(&self) -> usize {
        MAX_LIST_VALUES
    }
}

/// The error of a statement of the file. A statement that the progress
/// handler stopped, `SQLite` code 9 (`SQLITE_INTERRUPT`), is [`OverBudget`].
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

fn bound(sql: &str, values: Vec<SqlValue>) -> Query<'_, Sqlite, SqliteArguments> {
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
    query
}

fn decode(row: &SqliteRow) -> anyhow::Result<SqlRow> {
    (0..row.len())
        .map(|index| -> anyhow::Result<SqlValue> {
            let raw = row.try_get_raw(index)?;
            if raw.is_null() {
                return Ok(SqlValue::Null);
            }
            let class = raw.type_info();
            Ok(match class.name() {
                "REAL" => SqlValue::Real(row.try_get(index)?),
                "TEXT" => SqlValue::Text(row.try_get(index)?),
                "BLOB" => SqlValue::Blob(row.try_get(index)?),
                _ => SqlValue::Integer(row.try_get(index)?),
            })
        })
        .collect()
}
