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
use crate::read_database::file::FileReadDatabase;
use crate::read_database::{
    OverBudget, ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement,
    Transaction, WriteWork,
};
use provenance_core::protocol::read_failure::ReadFailure;
use std::sync::Arc;

/// The rows that the adapter reports for a statement past its bound.
#[derive(Debug, thiserror::Error)]
#[error("the statements read more rows than the bound")]
struct PastTheBound;

/// The engine steps that [`CountsRows`] takes for one row read: the low
/// end of the steps per row of the measured page statements.
const STEPS_PER_ROW: u64 = 3;

/// The file adapter as a store that counts its work in rows read. The
/// first statement after the bound reads `scan` rows, as a scan of a
/// large scope does, and each later statement reads one row. It converts
/// a bound in engine steps to `bound / STEPS_PER_ROW` rows.
#[derive(Debug)]
struct CountsRows {
    file: FileReadDatabase,
    scan: u64,
}

/// A transaction of [`CountsRows`].
struct RowsTransaction {
    inner: Transaction,
    scan: u64,
    bound: Option<u64>,
    read: u64,
}

impl SqlTransaction for RowsTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            if let Some(bound) = self.bound {
                self.read += if self.read == 0 { self.scan } else { 1 };
                if self.read > bound {
                    return Err(OverBudget(PastTheBound.into()).into());
                }
            }
            self.inner.fetch(sql, values).await
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
        self.bound = Some(bound / STEPS_PER_ROW);
        Box::pin(async { Ok(()) })
    }

    fn max_list_values(&self) -> usize {
        self.inner.max_list_values()
    }
}

impl ReadDatabase for CountsRows {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        let scan = self.scan;
        self.file.read(Box::new(move |inner| {
            work(Box::new(RowsTransaction {
                inner,
                scan,
                bound: None,
                read: 0,
            }))
        }))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.file.write(work)
    }

    fn max_bound_values(&self) -> usize {
        self.file.max_bound_values()
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        self.file.cursor_key()
    }

    fn location(&self) -> String {
        self.file.location()
    }
}

/// One search page over [`CountsRows`] whose first statement reads
/// `scan` rows.
async fn search_reading(scan: u64) -> anyhow::Result<usize> {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let file = FileReadDatabase::new(store.layout());
    let repository = over(&store, Some(Arc::new(CountsRows { file, scan })));
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
