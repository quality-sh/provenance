//! The file adapter: the CLI's `SQLite` file under `.provenance/cache`.
//!
//! Each call opens the file, runs the work in one transaction, and
//! completes the connection before it gives its result, as
//! [`CacheConnection`] does. A read opens the stored file and does not
//! make it. A write makes the file when it is absent and uses WAL mode.

use super::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use crate::cache::{open_cache, open_stored_cache, CacheConnection};
use crate::layout::ProvenanceLayout;
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool, TypeInfo, ValueRef};
use std::io::{Read, Write};

/// The maximum number of values that one statement binds in the file.
pub const MAX_BOUND_VALUES: usize = 999;

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
            let connection = open_stored_cache(&self.layout).await?;
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
    Ok(Box::new(FileTransaction(pool.begin().await?)))
}

struct FileTransaction(sqlx::Transaction<'static, Sqlite>);

impl SqlTransaction for FileTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let rows = bound(sql, values).fetch_all(&mut *self.0).await?;
            rows.iter().map(decode).collect()
        })
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, u64> {
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
            self.execute("SAVEPOINT attempt", &[]).await?;
            let mut failure = None;
            for statement in statements {
                if let Err(error) = self.execute(&statement.sql, &statement.values).await {
                    failure = Some(error);
                    break;
                }
            }
            if failure.is_some() {
                self.execute("ROLLBACK TO attempt", &[]).await?;
            }
            self.execute("RELEASE attempt", &[]).await?;
            Ok(failure.map_or(Ok(()), Err))
        })
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        Box::pin(async move { Ok(self.0.commit().await?) })
    }

    fn sqlx(&mut self) -> Option<&mut sqlx::Transaction<'static, Sqlite>> {
        Some(&mut self.0)
    }

    fn sqlx_ref(&self) -> Option<&sqlx::Transaction<'static, Sqlite>> {
        Some(&self.0)
    }
}

fn bound<'q>(sql: &'q str, values: &[SqlValue]) -> Query<'q, Sqlite, SqliteArguments> {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for value in values {
        query = match value {
            SqlValue::Null => query.bind(None::<i64>),
            SqlValue::Integer(value) => query.bind(*value),
            SqlValue::Real(value) => query.bind(*value),
            SqlValue::Text(value) => query.bind(value.clone()),
            SqlValue::Blob(value) => query.bind(value.clone()),
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
            let class = raw.type_info().name().to_owned();
            Ok(match class.as_str() {
                "REAL" => SqlValue::Real(row.try_get(index)?),
                "TEXT" => SqlValue::Text(row.try_get(index)?),
                "BLOB" => SqlValue::Blob(row.try_get(index)?),
                _ => SqlValue::Integer(row.try_get(index)?),
            })
        })
        .collect()
}
