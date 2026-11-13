//! The commit case over a store that commits after the work and loses a
//! commit that fails. No Rule: the case must refuse this store.

use super::File;
use provenance_store::read_database::contract::{self, Harness};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use std::sync::{Arc, Mutex};

/// The file adapter with a fault. It commits each write after its work,
/// and when that commit fails, it reports success and keeps nothing.
#[derive(Debug)]
struct LosesCommit(FileReadDatabase);

/// A write transaction whose commit call succeeds and only gives the
/// transaction back to the store, which commits it after the work.
struct LaterCommit {
    transaction: Transaction,
    held: Arc<Mutex<Option<Transaction>>>,
}

impl SqlTransaction for LaterCommit {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        self.transaction.fetch(sql, values)
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        self.transaction.execute(sql, values)
    }

    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>> {
        self.transaction.attempt(statements)
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        let Self { transaction, held } = *self;
        Box::pin(async move {
            *held.lock().unwrap() = Some(transaction);
            Ok(())
        })
    }

    fn bound_work(&mut self, steps: u64) -> SqlFuture<'_, ()> {
        self.transaction.bound_work(steps)
    }

    fn max_list_values(&self) -> usize {
        self.transaction.max_list_values()
    }
}

impl ReadDatabase for LosesCommit {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0.read(work)
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0.write(Box::new(move |transaction| {
            Box::pin(async move {
                let held = Arc::new(Mutex::new(None));
                work(Box::new(LaterCommit {
                    transaction,
                    held: Arc::clone(&held),
                }))
                .await?;
                let transaction = held.lock().unwrap().take();
                if let Some(transaction) = transaction {
                    let _lost = transaction.commit().await;
                }
                Ok(())
            })
        }))
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

struct Losing {
    file: File,
    database: LosesCommit,
}

impl Harness for Losing {
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

/// The commit case finds a store whose work succeeds, whose commit after
/// the work fails, and whose write then reports success.
#[tokio::test]
#[should_panic(
    expected = "a write whose commit failed reported success; the commit call gave Some(Ok(()))"
)]
async fn the_commit_case_refuses_a_store_that_loses_a_failed_commit() {
    let file = File::default();
    let database = LosesCommit(file.database.clone());
    contract::sequence::a_write_whose_commit_fails_keeps_nothing(Losing { file, database }).await;
}
