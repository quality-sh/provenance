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
//! commits it at its end. The store completes its connection after the
//! work ends, so a lock that the work frees after its commit is free while
//! the connection closes.

pub mod contract;
#[cfg(feature = "host")]
pub mod file;

use std::any::Any;
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

/// The transaction that a piece of work runs in. It is `Any` so that the
/// crate can find the transaction of its own file adapter.
pub trait SqlTransaction: Any + Send {
    /// Runs one statement and gives its rows.
    fn fetch<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, Vec<SqlRow>>;

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

    /// Commits the transaction. Write work calls this at its end.
    fn commit(self: Box<Self>) -> SqlFuture<'static, ()>;
}

/// A transaction lent to one piece of work.
pub type Transaction = Box<dyn SqlTransaction>;

/// Work over a read transaction. The transaction ends when the work drops
/// it.
pub type ReadWork<'a> = Box<dyn FnOnce(Transaction) -> SqlFuture<'a, ()> + Send + 'a>;

/// Work over a write transaction. The work commits the transaction. A
/// work that fails, or that ends without the commit, keeps nothing.
pub type WriteWork<'a> = Box<dyn FnOnce(Transaction) -> SqlFuture<'a, ()> + Send + 'a>;

/// The read database of one repository.
pub trait ReadDatabase: Send + Sync + Debug {
    /// Runs `work` in one read transaction. Each row that the work reads
    /// is from one committed state of the database. When the store cannot
    /// open the database, the error is [`Unopened`] and comes before the
    /// work starts.
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()>;

    /// Runs `work` in one write transaction, and makes the database when
    /// it is absent. The work commits the transaction; when the work fails
    /// or ends without the commit, the transaction rolls back.
    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()>;

    /// Runs `write` as [`ReadDatabase::write`] does and, when it succeeds,
    /// `read` as [`ReadDatabase::read`] does, over one open of the store.
    /// The store completes its connection once, after the read, so a
    /// failure of that completion is the failure of the read. When `write`
    /// fails, `read` does not run.
    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            self.write(write).await?;
            self.read(read).await
        })
    }

    /// The maximum number of values that one statement can bind.
    fn max_bound_values(&self) -> usize;

    /// The key that signs read cursors. It stays the same for the life of
    /// the store of the database.
    fn cursor_key(&self) -> anyhow::Result<[u8; 32]>;

    /// Where the database is, for the text of a refusal.
    fn location(&self) -> String;
}

/// The store could not open the database, so the work did not start. A
/// read of an absent database fails with this error, and the store makes
/// nothing. Its text is the text of its cause.
#[derive(Debug, thiserror::Error)]
#[error("{0:#}")]
pub struct Unopened(pub anyhow::Error);

impl Unopened {
    /// The cause, when `error` says that the store could not open the
    /// database; otherwise `error` itself.
    pub fn cause(error: anyhow::Error) -> Result<anyhow::Error, anyhow::Error> {
        error.downcast::<Self>().map(|unopened| unopened.0)
    }
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
