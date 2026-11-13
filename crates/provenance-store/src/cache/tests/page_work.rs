//! The work bound of a page over an adapter that reports the bound when a
//! statement ends, or that counts its work in rows read.
//!
//! No Rule: this is an implementation aid. The transaction under test is a
//! wrapper over the file that makes the `OverBudget` error and records the
//! bound, so these tests hold the page code only, not what an adapter
//! does at the bound. The shared contract case
//! `limits::a_statement_far_past_the_bound_gives_over_budget` holds that.

use super::fixtures::{open, seeded_layout};
use crate::cache::read::page::{page_error, PAGE_ROWS_READ, PAGE_WORK_STEPS};
use crate::cache::{materialize_state, open_cache};
use crate::operations::reader::ReadSnapshot;
use crate::read_database::{
    OverBudget, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction, WorkUnit,
};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::Requirement;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// An error that is not an error of the file, below [`OverBudget`].
#[derive(Debug, thiserror::Error)]
#[error("the store reported the statement at its end")]
struct Reported;

/// A transaction over the file that counts work in `unit` and records the
/// last bound that it got. After [`SqlTransaction::bound_work`], each
/// statement runs to its end and then gives `OverBudget(Reported)`.
struct ReportsAtEnd {
    inner: Transaction,
    unit: WorkUnit,
    bound: Arc<AtomicU64>,
    bounded: bool,
}

impl SqlTransaction for ReportsAtEnd {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let rows = self.inner.fetch(sql, values).await?;
            if self.bounded {
                return Err(OverBudget(Reported.into()).into());
            }
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
        self.bounded = true;
        self.bound.store(bound, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }

    fn work_unit(&self) -> WorkUnit {
        self.unit
    }

    fn max_list_values(&self) -> usize {
        self.inner.max_list_values()
    }
}

/// Bounds one page over [`ReportsAtEnd`] in `unit` and reads its first
/// requirement ids. Gives the bound that the page set and the page.
async fn bounded_page(unit: WorkUnit) -> (u64, anyhow::Result<Vec<String>>) {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&open(&layout)).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    let bound = Arc::new(AtomicU64::new(0));
    let transaction = ReportsAtEnd {
        inner: crate::read_database::file::begin(cache.pool())
            .await
            .unwrap(),
        unit,
        bound: Arc::clone(&bound),
        bounded: false,
    };
    let snapshot = ReadSnapshot::open(Box::new(transaction), &scope)
        .await
        .unwrap()
        .expect("a revision");
    snapshot.bound_page_work().await.unwrap();
    let page = snapshot
        .table::<Requirement>()
        .page_ids("", 10, false, None)
        .await
        .map_err(page_error);
    drop(snapshot);
    cache.close().await.unwrap();
    (bound.load(Ordering::SeqCst), page)
}

/// A page statement that runs to its end and that the adapter then
/// reports with `OverBudget` gives the page budget refusal, as a statement
/// that the file stops does.
#[tokio::test]
async fn a_page_statement_reported_at_its_end_is_the_page_budget_refusal() {
    let (_, page) = bounded_page(WorkUnit::EngineSteps).await;
    let refused = page.unwrap_err();
    assert_eq!(
        refused.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{refused:#}"
    );
}

/// A page bounds its work in the unit that the adapter gives.
#[tokio::test]
async fn a_page_bounds_its_work_in_the_unit_of_the_adapter() {
    let mut bounds = Vec::new();
    for unit in [WorkUnit::EngineSteps, WorkUnit::RowsRead] {
        bounds.push(bounded_page(unit).await.0);
    }
    assert_eq!(bounds, vec![PAGE_WORK_STEPS, PAGE_ROWS_READ]);
}
