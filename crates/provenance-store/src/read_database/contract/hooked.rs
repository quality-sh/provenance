//! An adapter over the store of another adapter that changes what a hook
//! names: the text of a fetch, the work bound, and the answer after a
//! fetch. The tests that need an adapter with another behaviour at the
//! work bound or at `LIMIT` use it.

use crate::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use std::borrow::Cow;
use std::fmt::Debug;

/// What a [`Hooked`] adapter changes. Each transaction gets its own copy.
pub trait Hook: Clone + Debug + Send + Sync + 'static {
    /// The text that the inner transaction runs for a fetch of `sql`.
    fn sql<'s>(&self, sql: &'s str) -> Cow<'s, str> {
        Cow::Borrowed(sql)
    }

    /// The bound that the inner transaction gets for `bound`, or `None`
    /// to give it no bound.
    fn bound(&mut self, bound: u64) -> Option<u64> {
        Some(bound)
    }

    /// Runs after the inner transaction gives `rows` for a fetch. An error
    /// takes the place of the rows.
    fn fetched<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        rows: &'a [SqlRow],
    ) -> SqlFuture<'a, ()> {
        let _ = (inner, rows);
        Box::pin(async { Ok(()) })
    }
}

/// The adapter `D` with the hook `H` on each transaction.
#[derive(Debug, Clone)]
pub struct Hooked<D, H>(pub D, pub H);

/// A transaction of a [`Hooked`] adapter.
pub struct HookedTransaction<H> {
    inner: Transaction,
    hook: H,
}

impl<H: Hook> HookedTransaction<H> {
    /// `inner` with `hook`.
    pub const fn new(inner: Transaction, hook: H) -> Self {
        Self { inner, hook }
    }
}

impl<H: Hook> SqlTransaction for HookedTransaction<H> {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let text = self.hook.sql(sql);
            let rows = self.inner.fetch(&text, values).await?;
            self.hook.fetched(&mut self.inner, &rows).await?;
            Ok(rows)
        })
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        self.inner.execute(sql, values)
    }

    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>> {
        self.inner.attempt(statements)
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        self.inner.commit()
    }

    fn bound_work(&mut self, bound: u64) -> SqlFuture<'_, ()> {
        match self.hook.bound(bound) {
            Some(bound) => self.inner.bound_work(bound),
            None => Box::pin(async { Ok(()) }),
        }
    }

    fn max_list_values(&self) -> usize {
        self.inner.max_list_values()
    }
}

impl<D: ReadDatabase, H: Hook> ReadDatabase for Hooked<D, H> {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        let hook = self.1.clone();
        self.0.read(Box::new(move |lent| {
            work(Box::new(HookedTransaction::new(lent, hook)))
        }))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0.write(work)
    }

    fn max_bound_values(&self) -> usize {
        self.0.max_bound_values()
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        self.0.cursor_key()
    }

    fn location(&self) -> String {
        self.0.location()
    }
}
