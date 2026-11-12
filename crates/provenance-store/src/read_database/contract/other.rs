//! Another adapter over the store of an adapter: it runs the same SQL, but
//! its transaction is its own type, so the crate finds no `SQLx`
//! transaction in it, as with an adapter over a store that is not a local
//! file. Code that passes over it needs nothing but the port.

use crate::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};

/// The adapter `D` behind a transaction of another type.
#[derive(Debug, Clone)]
pub struct Other<D>(pub D);

/// A transaction of `D`, behind its own type.
pub struct OtherTransaction(pub Transaction);

impl SqlTransaction for OtherTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        self.0.fetch(sql, values)
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        self.0.execute(sql, values)
    }

    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>> {
        self.0.attempt(statements)
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        self.0.commit()
    }

    fn bound_work(&mut self, steps: u64) -> SqlFuture<'_, ()> {
        self.0.bound_work(steps)
    }

    fn max_list_values(&self) -> usize {
        self.0.max_list_values()
    }
}

fn other(transaction: Transaction) -> Transaction {
    Box::new(OtherTransaction(transaction))
}

impl<D: ReadDatabase> ReadDatabase for Other<D> {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0.read(Box::new(move |lent| work(other(lent))))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0.write(Box::new(move |lent| work(other(lent))))
    }

    fn write_then_read<'a>(
        &'a self,
        write: WriteWork<'a>,
        read: ReadWork<'a>,
    ) -> SqlFuture<'a, ()> {
        self.0.write_then_read(
            Box::new(move |lent| write(other(lent))),
            Box::new(move |lent| read(other(lent))),
        )
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
