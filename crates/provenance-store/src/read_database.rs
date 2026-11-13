//! The read database port: the SQL store that the projection lives in.
//!
//! The engine runs SQL text through this port and decodes the rows that it
//! gets back. The named queries stay above the port, as the snapshot
//! handles and the catch-up. Each store has an adapter. The file adapter
//! keeps the CLI's `SQLite` file under `.provenance/cache`.
//!
//! A transaction is lent to one piece of work and ends when the work ends.
//! The work owns the transaction while it runs: a read drops it, and a write
//! commits it at its end. The store completes its connection after the
//! work ends, so a lock that the work frees after its commit is free while
//! the connection closes.

pub mod contract;
#[cfg(feature = "host")]
pub mod file;
#[cfg(test)]
mod tests;

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
///
/// A statement owns its bound values, so a store can bind them without a
/// copy.
pub trait SqlTransaction: Any + Send {
    /// Runs one statement and gives its rows.
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>>;

    /// Runs one statement and gives the number of rows that it changed.
    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64>;

    /// Runs `statements` as one unit inside the transaction: all of them,
    /// or, when one fails, none of them. The inner error is the error of
    /// the statement that failed, and the transaction continues. The outer
    /// error means that the transaction itself failed.
    ///
    /// When a statement gives [`OverBudget`] and the adapter ends the
    /// transaction, as [`SqlTransaction::bound_work`] lets it, the outer
    /// error is that [`OverBudget`].
    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>>;

    /// Commits the transaction. Write work calls this at its end.
    fn commit(self: Box<Self>) -> SqlFuture<'static, ()>;

    /// Bounds the work of the statements that follow in this transaction.
    ///
    /// The adapter counts `bound` in the unit that
    /// [`SqlTransaction::work_unit`] gives, over all these statements. It
    /// can count work at whole check intervals: then the work of a
    /// statement after its last check is not counted, and a statement that
    /// does less work than one interval can run past the bound. The file
    /// adapter checks each 1,000 engine steps of one statement text, over
    /// all the runs of that text on its connection.
    ///
    /// When the counted work passes the bound during a statement, the
    /// statement gives [`OverBudget`], and the caller does not get its
    /// rows. The adapter can stop the statement while it runs, or let it
    /// run to its end and then report it.
    ///
    /// When the statement that gives [`OverBudget`] only reads, the
    /// transaction stays open: the work of the earlier statements is kept,
    /// and a later statement gives its rows or [`OverBudget`].
    ///
    /// When the statement that gives [`OverBudget`] writes, the adapter can
    /// end the transaction. Then the work of the transaction is not kept,
    /// and its commit fails. A later statement is then not in the
    /// transaction: the file adapter runs it outside any transaction, so a
    /// later write is kept although the commit fails. Work that writes
    /// after a bound must therefore stop at the first [`OverBudget`].
    fn bound_work(&mut self, bound: u64) -> SqlFuture<'_, ()>;

    /// The unit in which this adapter counts work for the work bound.
    ///
    /// No one factor converts one unit into the other, so a caller keeps a
    /// bound in each unit. Without this method, the unit is
    /// [`WorkUnit::EngineSteps`].
    fn work_unit(&self) -> WorkUnit {
        WorkUnit::EngineSteps
    }

    /// The most values of one list that a reader binds in one statement;
    /// a longer list goes in more statements. With the two values that a
    /// reader binds beside the list, it is at most
    /// [`ReadDatabase::max_bound_values`].
    fn max_list_values(&self) -> usize;
}

/// What an adapter counts as the work of a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkUnit {
    /// The steps of the `SQLite` engine (virtual machine instructions), as
    /// its progress handler counts them. The file adapter counts these.
    EngineSteps,
    /// The rows that the store read from its tables and indexes for the
    /// statement, for a store that counts work only in rows.
    RowsRead,
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
    ///
    /// The write fails when its commit fails, also when the store makes the
    /// commit durable only after the work has ended. Then nothing that the
    /// work wrote is kept, and the caller does not use what the work
    /// reported.
    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()>;

    /// Runs `write` and then, when it succeeds, `read`.
    ///
    /// Each runs as [`ReadDatabase::write`] and [`ReadDatabase::read`] do,
    /// and the read sees what the write committed.
    ///
    /// The outer error is the failure of the write, its commit included;
    /// then `read` does not run. The inner error is the failure of the
    /// read. An adapter can run both over one open of its store, and then
    /// a failure to complete that open after the read is the failure of the
    /// read.
    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            self.write(write).await?;
            Ok(self.read(read).await)
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

/// The work of a transaction passed its work bound during a statement.
///
/// The bound is the one that [`SqlTransaction::bound_work`] set. The
/// statement gives no rows: the store stopped it, or reported it at its
/// end. Its text is the text of its cause.
#[derive(Debug, thiserror::Error)]
#[error("{0:#}")]
pub struct OverBudget(pub anyhow::Error);

/// The one row of a statement that gives exactly one, as an aggregate does.
pub fn one_row(rows: &[SqlRow]) -> anyhow::Result<&[SqlValue]> {
    rows.first().map(Vec::as_slice).ok_or_else(|| {
        crate::refusal::fault!(
            "no rows returned by a query that expected to return at least one row"
        )
    })
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
    /// The text of one column, moved out of the row without a copy. The
    /// column holds empty text after.
    fn take_text(&mut self, index: usize) -> anyhow::Result<String>;
    /// The text or NULL of one column, moved out of the row without a
    /// copy. The column holds empty text after.
    fn take_optional_text(&mut self, index: usize) -> anyhow::Result<Option<String>>;
    /// The bytes of one blob column.
    fn blob(&self, index: usize) -> anyhow::Result<&[u8]>;
}

impl SqlColumns for [SqlValue] {
    fn integer(&self, index: usize) -> anyhow::Result<i64> {
        match self.get(index) {
            Some(SqlValue::Integer(value)) => Ok(*value),
            _ => Err(column_fault(self, index, "an integer")),
        }
    }

    fn text(&self, index: usize) -> anyhow::Result<&str> {
        match self.get(index) {
            Some(SqlValue::Text(value)) => Ok(value),
            _ => Err(column_fault(self, index, "text")),
        }
    }

    fn optional_text(&self, index: usize) -> anyhow::Result<Option<&str>> {
        match self.get(index) {
            Some(SqlValue::Null) => Ok(None),
            Some(SqlValue::Text(value)) => Ok(Some(value)),
            _ => Err(column_fault(self, index, "text or null")),
        }
    }

    fn take_text(&mut self, index: usize) -> anyhow::Result<String> {
        if let Some(SqlValue::Text(text)) = self.get_mut(index) {
            return Ok(std::mem::take(text));
        }
        Err(column_fault(self, index, "text"))
    }

    fn take_optional_text(&mut self, index: usize) -> anyhow::Result<Option<String>> {
        match self.get(index) {
            Some(SqlValue::Null) => Ok(None),
            Some(SqlValue::Text(_)) => self.take_text(index).map(Some),
            _ => Err(column_fault(self, index, "text or null")),
        }
    }

    fn blob(&self, index: usize) -> anyhow::Result<&[u8]> {
        match self.get(index) {
            Some(SqlValue::Blob(value)) => Ok(value),
            _ => Err(column_fault(self, index, "a blob")),
        }
    }
}

fn column_fault(row: &[SqlValue], index: usize, wanted: &str) -> anyhow::Error {
    let Some(found) = row.get(index) else {
        return crate::refusal::fault!(
            "column index out of bounds: the len is {}, but the index is {index}",
            row.len()
        );
    };
    let class = match found {
        SqlValue::Null => "a NULL value",
        SqlValue::Integer(_) => "an INTEGER value",
        SqlValue::Real(_) => "a REAL value",
        SqlValue::Text(_) => "a TEXT value",
        SqlValue::Blob(_) => "a BLOB value",
    };
    crate::refusal::fault!("column {index} holds {class}, not {wanted}")
}
