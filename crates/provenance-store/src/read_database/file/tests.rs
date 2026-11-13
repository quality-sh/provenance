//! The work-bound cases of the contract over an adapter that counts its
//! work in rows read, and the margin of the case within the bound on the
//! file adapter.
//!
//! No Rule: an implementation aid. It holds that the cases accept an
//! honest adapter with another measure of work, as the port doc states.

use super::{FileReadDatabase, FileTransaction};
use crate::layout::ProvenanceLayout;
use crate::read_database::contract::{self, Harness, Hook, Hooked};
use crate::read_database::{OverBudget, ReadDatabase, SqlFuture, SqlRow, SqlValue, Transaction};
use libsqlite3_sys as sqlite;

/// The engine steps that [`RowsRead`] takes for one row read: the low end
/// of the steps per row of the measured page statements.
const STEPS_PER_ROW: u64 = 3;

/// The file adapter as a store that counts its work in rows read: the
/// rows that its statements step over in full scans of a table or an
/// index, and the rows that they give. It lets each statement run to its
/// end, and gives [`OverBudget`] when the rows read since the bound pass
/// `bound / STEPS_PER_ROW`.
#[derive(Debug, Clone, Default)]
struct RowsRead {
    bound: Option<u64>,
    read: u64,
}

impl Hook for RowsRead {
    fn bound(&mut self, bound: u64) -> Option<u64> {
        self.bound = Some(bound / STEPS_PER_ROW);
        self.read = 0;
        None
    }

    fn fetched<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        rows: &'a [SqlRow],
    ) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            let scanned = full_scan_rows(inner).await?;
            self.read += scanned + u64::try_from(rows.len())?;
            match self.bound {
                Some(bound) if self.read > bound => Err(OverBudget(anyhow::anyhow!(
                    "{} rows read pass the bound of {bound} rows",
                    self.read
                ))
                .into()),
                _ => Ok(()),
            }
        })
    }
}

/// The rows that the statements of the connection of `inner` stepped over
/// in full scans since the last call. Each call sets the counts to zero.
async fn full_scan_rows(inner: &mut Transaction) -> anyhow::Result<u64> {
    let file = (inner.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<FileTransaction>()
        .expect("a transaction of the file");
    let mut handle = file.0.lock_handle().await?;
    let database = handle.as_raw_handle().as_ptr();
    let mut total = 0;
    // The locked handle keeps the connection for this call alone, and each
    // statement pointer comes from the connection.
    unsafe {
        let mut statement = sqlite::sqlite3_next_stmt(database, std::ptr::null_mut());
        while !statement.is_null() {
            let steps =
                sqlite::sqlite3_stmt_status(statement, sqlite::SQLITE_STMTSTATUS_FULLSCAN_STEP, 1);
            total += u64::try_from(steps)?;
            statement = sqlite::sqlite3_next_stmt(database, statement);
        }
    }
    Ok(total)
}

/// A file database in a new cache directory of its own, with the hook `H`.
struct File<H> {
    _root: tempfile::TempDir,
    database: Hooked<FileReadDatabase, H>,
}

impl<H: Hook> File<H> {
    fn new(hook: H) -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(path);
        std::fs::create_dir_all(layout.cache_dir()).unwrap();
        Self {
            database: Hooked(FileReadDatabase::new(layout), hook),
            _root: root,
        }
    }
}

impl<H: Hook> Harness for File<H> {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        Box::new(self.database.clone())
    }

    fn fail_commit<'a>(&'a self, _transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        panic!("the work-bound cases do not fail a commit")
    }
}

/// The file adapter with no change.
#[derive(Debug, Clone)]
struct Unchanged;

impl Hook for Unchanged {}

#[tokio::test]
async fn the_work_bound_cases_accept_an_adapter_that_counts_rows_read() {
    contract::limits::a_bounded_read_runs_the_statements_within_its_bound(File::new(
        RowsRead::default(),
    ))
    .await;
    contract::limits::a_statement_far_past_the_bound_gives_over_budget(File::new(
        RowsRead::default(),
    ))
    .await;
    contract::limits::a_statement_well_within_the_bound_gives_its_rows(File::new(
        RowsRead::default(),
    ))
    .await;
}

/// The statement of the case within the bound, in one read of `file`
/// after the bound `bound`.
async fn within(file: &File<Unchanged>, bound: u64) -> anyhow::Result<Vec<SqlRow>> {
    let mut rows = None;
    let slot = &mut rows;
    file.database()
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                transaction.bound_work(bound).await?;
                *slot = Some(
                    transaction
                        .fetch(contract::limits::WITHIN, Vec::new())
                        .await,
                );
                Ok(())
            })
        }))
        .await?;
    rows.expect("the read ran its work")
}

/// The file adapter answers the statement of the case within the bound
/// in one twentieth of that bound, the margin of the port doc, and stops
/// it at 10,000 engine steps, so the statement does real work.
#[tokio::test]
async fn the_file_answers_the_statement_within_the_bound_in_a_twentieth_of_it() {
    let file = File::new(Unchanged);
    contract::limits::tall(file.database()).await;
    let answered = within(&file, contract::limits::WITHIN_BOUND / 20)
        .await
        .unwrap();
    assert_eq!(answered, vec![vec![SqlValue::Integer(50_005_000)]]);
    let stopped = within(&file, 10_000).await.unwrap_err();
    assert!(
        stopped.downcast_ref::<OverBudget>().is_some(),
        "not OverBudget: {stopped:#}"
    );
}
