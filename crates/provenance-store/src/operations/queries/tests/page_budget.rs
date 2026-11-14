//! The work bound of a page. A statement past it stops with the page
//! budget refusal, at the same count as before the bound went through the
//! port, on the file and on another transaction type over the file. The
//! page mapping also gives the page budget refusal for an `OverBudget`
//! whose cause is not an error of the file.
//!
//! No Rule: this is an implementation aid. The count is a characterization
//! of the bundled `SQLite` engine, recorded before the bound moved onto the
//! port, and the test reads private surfaces. The Rule's examples are the
//! page operation tests in `page_interrupt.rs`.

use super::comparison::test_stores::TestStore;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::reader::{page_error, ReadSnapshot};
use crate::read_database::contract::limits::COUNT_TO;
use crate::read_database::contract::OtherTransaction;
use crate::read_database::{
    OverBudget, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::Requirement;
use sqlx::SqlitePool;

/// The smallest count that the bound of one page stops, found by the
/// recorded run before the bound went through the port.
const FIRST_STOPPED_COUNT: i64 = 588_280;

/// Whether the bound of one page stops the count to `n`. Any other error
/// fails the test; the refusal is the page budget refusal.
async fn stops(pool: &SqlitePool, store: &TestStore, port_only: bool, n: i64) -> bool {
    let mut transaction = crate::read_database::file::begin(pool).await.unwrap();
    if port_only {
        transaction = OtherTransaction::forward(transaction);
    }
    let snapshot = ReadSnapshot::open(transaction, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    snapshot.bound_page_work().await.unwrap();
    match snapshot.fetch(COUNT_TO, vec![SqlValue::Integer(n)]).await {
        Ok(_) => false,
        Err(error) => {
            let error = page_error(error);
            assert_eq!(
                error.downcast_ref::<ReadFailure>(),
                Some(&ReadFailure::PageBudgetExceeded),
                "{error:#}"
            );
            assert_eq!(
                format!("{error:#}"),
                ReadFailure::PageBudgetExceeded.to_string()
            );
            true
        }
    }
}

/// The smallest count that the bound stops.
async fn first_stopped(pool: &SqlitePool, store: &TestStore, port_only: bool) -> i64 {
    let (mut low, mut high) = (1, 10_000_000);
    assert!(stops(pool, store, port_only, high).await);
    while low < high {
        let middle = low + (high - low) / 2;
        if stops(pool, store, port_only, middle).await {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[tokio::test]
async fn the_page_bound_stops_a_statement_at_the_same_count() {
    let store = TestStore::pinned();
    catch_up_state(&store.repository()).await.unwrap();
    let cache = open_cache(&store.layout()).await.unwrap();
    let file = first_stopped(cache.pool(), &store, false).await;
    let port_only = first_stopped(cache.pool(), &store, true).await;
    cache.close().await.unwrap();
    println!("=== FIRST STOPPED COUNT {file}");
    assert_eq!(
        (file, port_only),
        (FIRST_STOPPED_COUNT, FIRST_STOPPED_COUNT)
    );
}

/// An error that is not an error of the file, below [`OverBudget`].
#[derive(Debug, thiserror::Error)]
#[error("the store stopped the statement")]
struct Stopped;

/// A transaction over the file that, after
/// [`SqlTransaction::bound_work`], gives `OverBudget(Stopped)` for each
/// statement and runs none of them.
struct Stopping {
    inner: Transaction,
    bounded: bool,
}

impl SqlTransaction for Stopping {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        if self.bounded {
            Box::pin(async { Err(OverBudget(Stopped.into()).into()) })
        } else {
            self.inner.fetch(sql, values)
        }
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

    fn bound_work(&mut self, _steps: u64) -> SqlFuture<'_, ()> {
        self.bounded = true;
        Box::pin(async { Ok(()) })
    }

    fn max_list_values(&self) -> usize {
        self.inner.max_list_values()
    }
}

/// A list page gives its `OverBudget` error through to [`page_error`],
/// which gives the page budget refusal for it. This holds the mapping
/// only: the transaction makes the error, so the test says nothing about
/// what an adapter does at the bound.
#[tokio::test]
async fn the_page_mapping_refuses_any_over_budget_error() {
    let store = TestStore::pinned();
    catch_up_state(&store.repository()).await.unwrap();
    let cache = open_cache(&store.layout()).await.unwrap();
    let inner = crate::read_database::file::begin(cache.pool())
        .await
        .unwrap();
    let stopping = Box::new(Stopping {
        inner,
        bounded: false,
    });
    let snapshot = ReadSnapshot::open(stopping, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    snapshot.bound_page_work().await.unwrap();
    let refused = snapshot
        .table::<Requirement>()
        .page_ids("", 10, false, None)
        .await
        .map_err(page_error)
        .unwrap_err();
    drop(snapshot);
    cache.close().await.unwrap();
    assert_eq!(
        refused.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{refused:#}"
    );
}
