//! A store that counts its work in rows read, over the file: the shared
//! work-bound cases accept it.
//!
//! No Rule: an implementation aid. The port doc lets an adapter convert the
//! bound to rows read; this adapter does so, and these tests show that the
//! cases do not refuse such an adapter. It counts the rows that `SQLite`
//! steps through in full scans and the rows that a statement gives, and it
//! reports the bound when a statement ends.

use super::{FileReadDatabase, FileTransaction};
use crate::layout::ProvenanceLayout;
use crate::read_database::contract::{limits, Harness, Hooks, MakeHooks, Other};
use crate::read_database::{OverBudget, ReadDatabase, SqlFuture, SqlRow, SqlValue, Transaction};

/// The engine steps that the adapter takes for one row read: the low end
/// of the steps per row of the measured table reads.
const STEPS_PER_ROW: u64 = 3;

/// The error of a statement past the bound.
#[derive(Debug, thiserror::Error)]
#[error("the statements read {read} rows, past the bound of {bound} rows")]
struct PastTheBound {
    read: u64,
    bound: u64,
}

/// Makes the counter of each transaction.
#[derive(Debug, Clone, Copy)]
struct RowsRead;

/// The rows that the statements of one transaction read after its bound.
struct Counter {
    bound: Option<u64>,
    read: u64,
}

impl MakeHooks for RowsRead {
    type Hooks = Counter;
    fn make(&self) -> Counter {
        Counter {
            bound: None,
            read: 0,
        }
    }
}

/// The full-scan steps of the statements of the connection of `inner`
/// since the last call. The call sets each count to zero.
async fn full_scan_rows(inner: &mut Transaction) -> anyhow::Result<u64> {
    let file = (inner.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<FileTransaction>()
        .ok_or_else(|| {
            crate::refusal::fault!("the transaction is not a transaction of the file")
        })?;
    let mut handle = file.0.lock_handle().await?;
    let connection = handle.as_raw_handle().as_ptr();
    let mut rows = 0;
    // SAFETY: the locked handle keeps every other use off the connection
    // and its statements while this loop runs, and each statement pointer
    // comes from `sqlite3_next_stmt` on that connection.
    let mut statement =
        unsafe { libsqlite3_sys::sqlite3_next_stmt(connection, std::ptr::null_mut()) };
    while !statement.is_null() {
        // SAFETY: as above.
        let steps = unsafe {
            libsqlite3_sys::sqlite3_stmt_status(
                statement,
                libsqlite3_sys::SQLITE_STMTSTATUS_FULLSCAN_STEP,
                1,
            )
        };
        rows += u64::try_from(steps)?;
        // SAFETY: as above.
        statement = unsafe { libsqlite3_sys::sqlite3_next_stmt(connection, statement) };
    }
    Ok(rows)
}

impl Hooks for Counter {
    fn fetch<'a>(
        &'a mut self,
        inner: &'a mut Transaction,
        sql: &'a str,
        values: Vec<SqlValue>,
    ) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move {
            let rows = inner.fetch(sql, values).await?;
            if let Some(bound) = self.bound {
                self.read += full_scan_rows(inner).await? + u64::try_from(rows.len())?;
                if self.read > bound {
                    let past = PastTheBound {
                        read: self.read,
                        bound,
                    };
                    return Err(OverBudget(past.into()).into());
                }
            }
            Ok(rows)
        })
    }

    fn bound_work<'a>(&'a mut self, inner: &'a mut Transaction, bound: u64) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            full_scan_rows(inner).await?;
            self.bound = Some(bound / STEPS_PER_ROW);
            self.read = 0;
            Ok(())
        })
    }
}

/// The rows-read store in a new cache directory of its own.
struct Rows {
    _root: tempfile::TempDir,
    layout: ProvenanceLayout,
    database: Other<FileReadDatabase, RowsRead>,
}

impl Rows {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(path);
        std::fs::create_dir_all(layout.cache_dir()).unwrap();
        Self {
            database: Other {
                database: FileReadDatabase::new(layout.clone()),
                hooks: RowsRead,
            },
            layout,
            _root: root,
        }
    }
}

impl Harness for Rows {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        Box::new(Other {
            database: FileReadDatabase::new(self.layout.clone()),
            hooks: RowsRead,
        })
    }

    fn fail_commit<'a>(&'a self, _transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        Box::pin(async {
            Err(crate::refusal::fault!(
                "the work-bound cases make no commit fail"
            ))
        })
    }
}

#[tokio::test]
async fn a_rows_read_store_gives_over_budget_far_past_the_bound() {
    limits::a_statement_far_past_the_bound_gives_over_budget(Rows::new()).await;
}

#[tokio::test]
async fn a_rows_read_store_answers_a_statement_well_within_the_bound() {
    limits::a_statement_well_within_the_bound_gives_its_rows(Rows::new()).await;
}

/// The statements of `rows` after the bound `bound`: the answer of the
/// scan of the margin case.
async fn scan_under(rows: &Rows, bound: u64) -> anyhow::Result<Vec<SqlRow>> {
    let mut answer = None;
    let slot = &mut answer;
    rows.database()
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                transaction.bound_work(bound).await?;
                *slot = Some(transaction.fetch(limits::WITHIN, Vec::new()).await);
                Ok(())
            })
        }))
        .await
        .unwrap();
    answer.unwrap()
}

/// The adapter counts the rows of a full scan: the scan of the 10,000 rows
/// of the margin case passes a bound of 9,000 rows, and stays within a
/// bound of 11,000 rows.
#[tokio::test]
async fn a_rows_read_store_counts_the_rows_of_a_full_scan() {
    let rows = Rows::new();
    limits::tall(rows.database()).await;
    let refused = scan_under(&rows, 9_000 * STEPS_PER_ROW).await.unwrap_err();
    let text = format!("{refused:#}");
    assert!(refused.downcast_ref::<OverBudget>().is_some(), "{text}");
    assert!(
        text.starts_with("the statements read 1000")
            && text.ends_with(" rows, past the bound of 9000 rows"),
        "{text}"
    );
    let answered = scan_under(&rows, 11_000 * STEPS_PER_ROW).await.unwrap();
    assert_eq!(answered, vec![vec![SqlValue::Integer(50_005_000)]]);
}
