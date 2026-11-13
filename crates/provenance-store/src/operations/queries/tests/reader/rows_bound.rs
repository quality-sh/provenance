//! The work bound of a page over an adapter that counts its work in rows
//! read and converts the bound in engine steps itself.
//!
//! No Rule: an implementation aid. It holds that the page gives such an
//! adapter the bound `PAGE_WORK_STEPS`, and that the page budget refusal
//! comes from the `OverBudget` of the adapter's own conversion.

use super::super::comparison::requests;
use super::super::comparison::test_stores;
use super::adapter::over;
use super::get_through;
use crate::operations::queries::records;
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::answer;
use crate::read_database::contract::{Hook, Hooked};
use crate::read_database::file::FileReadDatabase;
use crate::read_database::{OverBudget, SqlFuture, SqlRow, Transaction};
use provenance_core::protocol::read_failure::ReadFailure;
use std::sync::Arc;

/// The rows that the adapter reports for a statement past its bound.
#[derive(Debug, thiserror::Error)]
#[error("the statements read more rows than the bound")]
struct PastTheBound;

/// The engine steps that [`CountsRows`] takes for one row read: the low
/// end of the steps per row of the measured page statements.
const STEPS_PER_ROW: u64 = 3;

/// A hook that makes the file adapter a store that counts its work in
/// rows read. The first statement after the bound reads `scan` rows, as a
/// scan of a large scope does, and each later statement reads one row. It
/// converts a bound in engine steps to `bound / STEPS_PER_ROW` rows, and
/// gives the file no bound.
#[derive(Debug, Clone)]
struct CountsRows {
    scan: u64,
    bound: Option<u64>,
    read: u64,
}

impl Hook for CountsRows {
    fn bound(&mut self, bound: u64) -> Option<u64> {
        self.bound = Some(bound / STEPS_PER_ROW);
        None
    }

    fn fetched<'a>(
        &'a mut self,
        _inner: &'a mut Transaction,
        _rows: &'a [SqlRow],
    ) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            if let Some(bound) = self.bound {
                self.read += if self.read == 0 { self.scan } else { 1 };
                if self.read > bound {
                    return Err(OverBudget(PastTheBound.into()).into());
                }
            }
            Ok(())
        })
    }
}

/// One search page over [`CountsRows`] whose first statement reads
/// `scan` rows.
async fn search_reading(scan: u64) -> anyhow::Result<usize> {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let file = FileReadDatabase::new(store.layout());
    let hook = CountsRows {
        scan,
        bound: None,
        read: 0,
    };
    let repository = over(&store, Some(Arc::new(Hooked(file, hook))));
    let page = answer(&repository, ReadPolicy::default(), |ctx| {
        Box::pin(
            async move { records::search(ctx, requests::search("overtime", Vec::new())).await },
        )
    })
    .await?;
    Ok(page.result.nodes.len())
}

/// A page over an adapter that counts rows read answers when its
/// statements read 1,000,000 rows, and gives the page budget refusal when
/// they read 5,000,000: more than the 3,333,333 rows of its conversion of
/// `PAGE_WORK_STEPS`, and less than `PAGE_WORK_STEPS`.
#[tokio::test]
async fn a_page_bounds_the_rows_that_it_reads() {
    let answered = search_reading(1_000_000).await.unwrap();
    assert!(answered > 0, "the search found nothing");
    let refused = search_reading(5_000_000).await.unwrap_err();
    assert_eq!(
        refused.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{refused:#}"
    );
}
