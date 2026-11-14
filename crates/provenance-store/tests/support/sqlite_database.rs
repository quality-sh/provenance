//! A read database in one `SQLite` connection in memory, outside the store.
//!
//! The store has no read database of its own without `host`. A test that
//! runs the read operations without `host` uses this one, as an embedder
//! uses its own: through the port only. The connection lives as long as the
//! adapter, so the projection stays between calls.

use provenance_store::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool, TypeInfo, ValueRef};
use std::str::FromStr;

/// One `SQLite` database in memory.
#[derive(Debug, Clone)]
pub struct MemorySqlite(SqlitePool);

impl MemorySqlite {
    /// An empty database. It must be made on a Tokio runtime.
    pub async fn new() -> Self {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let pool = SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await
            .unwrap();
        Self(pool)
    }

    async fn begin(&self) -> anyhow::Result<Transaction> {
        Ok(Box::new(MemoryTransaction(self.0.begin().await?)))
    }
}

impl ReadDatabase for MemorySqlite {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move { work(self.begin().await?).await })
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move { work(self.begin().await?).await })
    }

    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            write(self.begin().await?).await?;
            Ok(read(self.begin().await?).await)
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
            let rows = bound(sql, values).fetch_all(&mut *self.0).await?;
            rows.iter().map(decode).collect()
        })
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        Box::pin(async move {
            Ok(bound(sql, values)
                .execute(&mut *self.0)
                .await?
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
            if failure.is_some() {
                self.execute("ROLLBACK TO attempt", Vec::new()).await?;
            }
            self.execute("RELEASE attempt", Vec::new()).await?;
            Ok(failure.map_or(Ok(()), Err))
        })
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        Box::pin(async move { Ok(self.0.commit().await?) })
    }

    /// The tests read small projections; no bound is set.
    fn bound_work(&mut self, _: u64) -> SqlFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn max_list_values(&self) -> usize {
        500
    }
}

fn bound(sql: &str, values: Vec<SqlValue>) -> sqlx::query::Query<'_, Sqlite, SqliteArguments> {
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
            Ok(match raw.type_info().name() {
                "REAL" => SqlValue::Real(row.try_get(index)?),
                "TEXT" => SqlValue::Text(row.try_get(index)?),
                "BLOB" => SqlValue::Blob(row.try_get(index)?),
                _ => SqlValue::Integer(row.try_get(index)?),
            })
        })
        .collect()
}
