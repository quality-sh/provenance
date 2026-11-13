//! The work bound of a page over an adapter that reports the bound when a
//! statement ends.
//!
//! No Rule: this is an implementation aid. The transaction under test is a
//! wrapper over the file that makes the `OverBudget` error and records the
//! bound, so these tests hold the page code only, not what an adapter
//! does at the bound. The shared contract case
//! `limits::a_statement_far_past_the_bound_gives_over_budget` holds that.

use super::fixtures::{open, seeded_layout};
use crate::cache::read::page::page_error;
use crate::cache::{materialize_state, open_cache};
use crate::operations::reader::ReadSnapshot;
use crate::read_database::contract::{Hooks, OtherTransaction};
use crate::read_database::{OverBudget, SqlFuture, SqlRow, SqlValue, Transaction};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::Requirement;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// An error that is not an error of the file, below [`OverBudget`].
#[derive(Debug, thiserror::Error)]
#[error("the store reported the statement at its end")]
struct Reported;

/// Hooks that record the last bound that the transaction got. After
/// [`Hooks::bound_work`], each statement runs to its end and then gives
/// `OverBudget(Reported)`.
struct ReportsAtEnd {
    bound: Arc<AtomicU64>,
    bounded: bool,
}

impl Hooks for ReportsAtEnd {
    fn fetch<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        sql: &'a str,
        values: Vec<SqlValue>,
    ) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let rows = inner.fetch(sql, values).await?;
            if self.bounded {
                return Err(OverBudget(Reported.into()).into());
            }
            Ok(rows)
        })
    }

    fn bound_work<'a>(&'a mut self, _inner: &'a mut Transaction, bound: u64) -> SqlFuture<'a, ()> {
        self.bounded = true;
        self.bound.store(bound, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}

/// Bounds one page over [`ReportsAtEnd`] and reads its first requirement
/// ids. Gives the bound that the page set and the page.
async fn bounded_page() -> (u64, anyhow::Result<Vec<String>>) {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&open(&layout)).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    let bound = Arc::new(AtomicU64::new(0));
    let transaction = OtherTransaction {
        inner: crate::read_database::file::begin(cache.pool())
            .await
            .unwrap(),
        hooks: ReportsAtEnd {
            bound: Arc::clone(&bound),
            bounded: false,
        },
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
    let (_, page) = bounded_page().await;
    let refused = page.unwrap_err();
    assert_eq!(
        refused.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{refused:#}"
    );
}

/// A page gives each adapter its bound in engine steps: 10,000,000.
#[tokio::test]
async fn a_page_bounds_its_work_in_engine_steps() {
    assert_eq!(bounded_page().await.0, 10_000_000);
}
