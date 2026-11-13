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
use crate::read_database::contract::{Hook, HookedTransaction};
use crate::read_database::{OverBudget, SqlFuture, SqlRow, Transaction};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::Requirement;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// An error that is not an error of the file, below [`OverBudget`].
#[derive(Debug, thiserror::Error)]
#[error("the store reported the statement at its end")]
struct Reported;

/// A hook that records the last bound that the transaction got and gives
/// the file no bound. After the bound, each statement runs to its end and
/// then gives `OverBudget(Reported)`.
#[derive(Debug, Clone)]
struct ReportsAtEnd {
    bound: Arc<AtomicU64>,
    bounded: bool,
}

impl Hook for ReportsAtEnd {
    fn bound(&mut self, bound: u64) -> Option<u64> {
        self.bounded = true;
        self.bound.store(bound, Ordering::SeqCst);
        None
    }

    fn fetched<'a>(
        &'a mut self,
        _inner: &'a mut Transaction,
        _rows: &'a [SqlRow],
    ) -> SqlFuture<'a, ()> {
        let bounded = self.bounded;
        Box::pin(async move {
            if bounded {
                return Err(OverBudget(Reported.into()).into());
            }
            Ok(())
        })
    }
}

/// Bounds one page over [`ReportsAtEnd`] and reads its first requirement
/// ids. Gives the bound that the page set and the page.
async fn bounded_page() -> (u64, anyhow::Result<Vec<String>>) {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&open(&layout)).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    let bound = Arc::new(AtomicU64::new(0));
    let transaction = HookedTransaction::new(
        crate::read_database::file::begin(cache.pool())
            .await
            .unwrap(),
        ReportsAtEnd {
            bound: Arc::clone(&bound),
            bounded: false,
        },
    );
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

/// A page gives each adapter the bound of 10,000,000 engine steps.
#[tokio::test]
async fn a_page_bounds_its_work_in_engine_steps() {
    assert_eq!(bounded_page().await.0, 10_000_000);
}
