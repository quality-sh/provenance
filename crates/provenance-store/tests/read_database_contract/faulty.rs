//! The file adapter with one fault, to prove that a contract case refuses
//! an adapter with that fault. No Rule: the cases must refuse these
//! adapters.

use super::File;
use provenance_store::read_database::contract::{Harness, Hooks, MakeHooks, Other};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::read_database::{
    OverBudget, ReadDatabase, SqlFuture, SqlRow, SqlValue, Transaction,
};

/// The fault of a [`Faulty`] adapter.
#[derive(Debug, Clone, Copy)]
pub enum Fault {
    /// It ignores the `LIMIT` of a statement.
    NoLimit,
    /// It ignores the work bound.
    NoBound,
    /// After a work bound, it gives `OverBudget` for every statement, so it
    /// refuses work that the file adapter answers by a wide margin.
    TooStrict,
}

/// The error of a [`Fault::TooStrict`] adapter.
#[derive(Debug, thiserror::Error)]
#[error("the adapter refuses each statement after a bound")]
struct Refused;

/// The hooks of one transaction of a [`Faulty`] adapter.
pub struct FaultyHooks {
    fault: Fault,
    bounded: bool,
}

impl MakeHooks for Fault {
    type Hooks = FaultyHooks;
    fn make(&self) -> FaultyHooks {
        FaultyHooks {
            fault: *self,
            bounded: false,
        }
    }
}

/// The statement with each `LIMIT n` read as no limit; the bound value of
/// the limit stays bound.
fn without_limit(sql: &str) -> String {
    let mut parts = sql.split(" LIMIT ");
    let mut text = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let end = part.find(char::is_whitespace).unwrap_or(part.len());
        let (limit, rest) = part.split_at(end);
        text = text + " LIMIT -1 + 0 * (" + limit + ")" + rest;
    }
    text
}

impl Hooks for FaultyHooks {
    fn fetch<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        sql: &'a str,
        values: Vec<SqlValue>,
    ) -> SqlFuture<'a, Vec<SqlRow>> {
        match self.fault {
            Fault::NoLimit => {
                Box::pin(async move { inner.fetch(&without_limit(sql), values).await })
            }
            Fault::TooStrict if self.bounded => {
                Box::pin(async { Err(OverBudget(Refused.into()).into()) })
            }
            Fault::NoBound | Fault::TooStrict => inner.fetch(sql, values),
        }
    }

    fn bound_work<'a>(&'a mut self, inner: &'a mut Transaction, bound: u64) -> SqlFuture<'a, ()> {
        self.bounded = true;
        match self.fault {
            Fault::NoBound => Box::pin(async { Ok(()) }),
            Fault::NoLimit | Fault::TooStrict => inner.bound_work(bound),
        }
    }
}

/// The file adapter with one fault.
pub struct Faulty {
    file: File,
    database: Other<FileReadDatabase, Fault>,
}

impl Faulty {
    pub fn new(fault: Fault) -> Self {
        let file = File::default();
        Self {
            database: Other {
                database: file.database.clone(),
                hooks: fault,
            },
            file,
        }
    }
}

impl Harness for Faulty {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        self.file.reopen()
    }

    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        self.file.fail_commit(transaction)
    }
}
