//! Another adapter over the store of an adapter: it runs the same SQL, but
//! its transaction is its own type. Code that passes over it needs nothing
//! but the port.
//!
//! The tests make faulty and converting adapters with it: [`Hooks`] change
//! what a transaction does for `fetch` and `bound_work`, and the wrapper
//! forwards every other call.

use crate::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use std::fmt::Debug;

/// What one transaction of [`Other`] does for `fetch` and `bound_work`.
/// Each call gets the transaction of the adapter below.
pub trait Hooks: Send + 'static {
    /// Runs one statement; the default forwards it.
    fn fetch<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        sql: &'a str,
        values: Vec<SqlValue>,
    ) -> SqlFuture<'a, Vec<SqlRow>> {
        inner.fetch(sql, values)
    }

    /// Sets the work bound; the default forwards it.
    fn bound_work<'a>(&'a mut self, inner: &'a mut Transaction, bound: u64) -> SqlFuture<'a, ()> {
        inner.bound_work(bound)
    }
}

/// Makes the [`Hooks`] of each transaction of [`Other`].
pub trait MakeHooks: Send + Sync + Debug {
    type Hooks: Hooks;
    fn make(&self) -> Self::Hooks;
}

/// Hooks that forward every call.
#[derive(Debug, Clone, Copy, Default)]
pub struct Forward;

impl Hooks for Forward {}

impl MakeHooks for Forward {
    type Hooks = Self;
    fn make(&self) -> Self {
        Self
    }
}

/// The adapter `database` behind a transaction of another type, with the
/// hooks that `hooks` makes for each transaction.
#[derive(Debug, Clone)]
pub struct Other<D, M = Forward> {
    pub database: D,
    pub hooks: M,
}

impl<D> Other<D> {
    /// `database` behind a transaction of another type that forwards every
    /// call.
    pub const fn new(database: D) -> Self {
        Self {
            database,
            hooks: Forward,
        }
    }
}

/// A transaction of the adapter below, behind its own type.
pub struct OtherTransaction<K = Forward> {
    pub inner: Transaction,
    pub hooks: K,
}

impl OtherTransaction {
    /// `inner` behind a transaction of another type that forwards every
    /// call.
    pub fn forward(inner: Transaction) -> Transaction {
        Box::new(Self {
            inner,
            hooks: Forward,
        })
    }
}

impl<K: Hooks> SqlTransaction for OtherTransaction<K> {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        self.hooks.fetch(&mut self.inner, sql, values)
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
        self.hooks.bound_work(&mut self.inner, bound)
    }

    fn max_list_values(&self) -> usize {
        self.inner.max_list_values()
    }
}

impl<D: ReadDatabase, M: MakeHooks> Other<D, M> {
    fn other(&self, inner: Transaction) -> Transaction {
        Box::new(OtherTransaction {
            inner,
            hooks: self.hooks.make(),
        })
    }
}

impl<D: ReadDatabase, M: MakeHooks> ReadDatabase for Other<D, M> {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.database
            .read(Box::new(move |lent| work(self.other(lent))))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.database
            .write(Box::new(move |lent| work(self.other(lent))))
    }

    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, anyhow::Result<()>> {
        self.database.write_then_read(
            Box::new(move |lent| write(self.other(lent))),
            Box::new(move |lent| read(self.other(lent))),
        )
    }

    fn max_bound_values(&self) -> usize {
        self.database.max_bound_values()
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        self.database.cursor_key()
    }

    fn location(&self) -> String {
        self.database.location()
    }
}
