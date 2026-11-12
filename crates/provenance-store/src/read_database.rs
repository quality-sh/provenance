//! The read database port: the SQL store that the projection lives in.
//!
//! The engine runs SQL text through this port and decodes the rows that it
//! gets back. The named queries stay above the port, as the snapshot
//! handles and the catch-up. Each store has an adapter: the file adapter
//! keeps the CLI's `SQLite` file under `.provenance/cache`, and a Durable
//! Object keeps the same tables in its own `SQLite`.
//!
//! A transaction is lent to one piece of work and ends when the work ends,
//! because the object's store runs a transaction over one closure. The
//! work owns the transaction while it runs: a read drops it, and a write
//! gives it back for the commit.

pub mod contract;
#[cfg(feature = "host")]
pub mod file;

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

/// One SQL value, by the storage class that `SQLite` gives for it.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<&str> for SqlValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for SqlValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for SqlValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for SqlValue {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(value)
    }
}

impl<T: Into<Self>> From<Option<T>> for SqlValue {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

/// One row, in the order of the select list.
pub type SqlRow = Vec<SqlValue>;

/// The future of one port call.
pub type SqlFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>;

/// One statement with its bound values.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub sql: String,
    pub values: Vec<SqlValue>,
}

/// The transaction that a piece of work runs in.
pub trait SqlTransaction: Send {
    /// Runs one statement and gives its rows.
    fn fetch<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue])
        -> SqlFuture<'a, Vec<SqlRow>>;

    /// Runs one statement and gives the number of rows that it changed.
    fn execute<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, u64>;

    /// Runs `statements` as one unit inside the transaction: all of them,
    /// or, when one fails, none of them. The inner error is the error of
    /// the statement that failed, and the transaction continues. The outer
    /// error means that the transaction itself failed.
    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>>;

    /// Commits the transaction. Only the adapter calls this, after write
    /// work gives the transaction back.
    fn commit(self: Box<Self>) -> SqlFuture<'static, ()>;

    /// The `SQLx` transaction of the file adapter, for the code that still
    /// runs `SQLx` directly. Other adapters have none.
    #[cfg(feature = "host")]
    fn sqlx(&mut self) -> Option<&mut sqlx::Transaction<'static, sqlx::Sqlite>> {
        None
    }
}

/// A transaction lent to one piece of work.
pub type Transaction = Box<dyn SqlTransaction>;

/// Work over a read transaction. The transaction ends when the work drops
/// it.
pub type ReadWork<'a> = Box<dyn FnOnce(Transaction) -> SqlFuture<'a, ()> + Send + 'a>;

/// Work over a write transaction. The work gives the transaction back to
/// be committed. An error rolls the transaction back.
pub type WriteWork<'a> = Box<dyn FnOnce(Transaction) -> SqlFuture<'a, Transaction> + Send + 'a>;

/// The read database of one repository.
pub trait ReadDatabase: Send + Sync + Debug {
    /// Runs `work` in one read transaction. Each row that the work reads
    /// is from one committed state of the database. When the store cannot
    /// open the database, the error comes before the work starts.
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()>;

    /// Runs `work` in one write transaction, and makes the database when
    /// it is absent. The transaction commits when the work gives it back,
    /// and rolls back when the work fails.
    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()>;

    /// The maximum number of values that one statement can bind.
    fn max_bound_values(&self) -> usize;

    /// The key that signs read cursors. It stays the same for the life of
    /// the store of the database.
    fn cursor_key(&self) -> anyhow::Result<[u8; 32]>;

    /// Where the database is, for the text of a refusal.
    fn location(&self) -> String;
}

/// Whether a store error says that a table that the statement names is
/// absent.
pub fn is_missing_table(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("no such table"))
}

/// Typed access to one column of a row.
pub trait SqlColumns {
    fn integer(&self, index: usize) -> anyhow::Result<i64>;
    fn text(&self, index: usize) -> anyhow::Result<&str>;
    fn optional_text(&self, index: usize) -> anyhow::Result<Option<&str>>;
}

impl SqlColumns for [SqlValue] {
    fn integer(&self, index: usize) -> anyhow::Result<i64> {
        match self.get(index) {
            Some(SqlValue::Integer(value)) => Ok(*value),
            other => Err(column_fault(index, "an integer", other)),
        }
    }

    fn text(&self, index: usize) -> anyhow::Result<&str> {
        match self.get(index) {
            Some(SqlValue::Text(value)) => Ok(value),
            other => Err(column_fault(index, "text", other)),
        }
    }

    fn optional_text(&self, index: usize) -> anyhow::Result<Option<&str>> {
        match self.get(index) {
            Some(SqlValue::Null) => Ok(None),
            Some(SqlValue::Text(value)) => Ok(Some(value)),
            other => Err(column_fault(index, "text or null", other)),
        }
    }
}

fn column_fault(index: usize, wanted: &str, found: Option<&SqlValue>) -> anyhow::Error {
    crate::refusal::fault!("column {index} holds {found:?}, not {wanted}")
}
