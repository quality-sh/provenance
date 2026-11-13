//! What the file adapter does with a statement that it stops at the work
//! bound, for a statement that reads, a statement that writes, and a
//! statement in an attempt. The port lets an adapter end the transaction
//! after a statement that writes gives `OverBudget`; these tests hold what
//! the file does.
//!
//! No Rule: an implementation aid. These tests hold what the doc of
//! `SqlTransaction::bound_work` states of the file adapter.

use super::File;
use provenance_store::read_database::{
    OverBudget, ReadDatabase, SqlFuture, SqlRow, SqlValue, Statement, Transaction,
};

/// A statement that writes a row for each of the million pairs of rows of
/// `wide`: far past a bound of 10,000 engine steps.
const WRITE_PAST: &str = "INSERT INTO kept (n) SELECT a.n FROM wide AS a, wide AS b";

/// A statement that reads each of the million pairs of rows of `wide`.
const READ_PAST: &str = "SELECT SUM(a.n * b.n) FROM wide AS a, wide AS b";

/// The bound of the tests, in engine steps.
const BOUND: u64 = 10_000;

/// Makes `wide`, with the integers 1 to 1,000, and an empty `kept`.
async fn tables(file: &File) {
    file.database
        .write(Box::new(|mut transaction| {
            Box::pin(async move {
                for sql in [
                    "CREATE TABLE wide (n INTEGER)",
                    "INSERT INTO wide (n) WITH RECURSIVE count_to (n) AS (SELECT 1 UNION ALL \
                     SELECT n + 1 FROM count_to WHERE n < 1000) SELECT n FROM count_to",
                    "CREATE TABLE kept (n INTEGER)",
                ] {
                    transaction.execute(sql, Vec::new()).await?;
                }
                transaction.commit().await
            })
        }))
        .await
        .unwrap();
}

/// The rows of `kept` in a later read.
async fn kept(file: &File) -> Vec<SqlRow> {
    let mut rows = Vec::new();
    let slot = &mut rows;
    file.database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                *slot = transaction
                    .fetch("SELECT n FROM kept ORDER BY n", Vec::new())
                    .await?;
                Ok(())
            })
        }))
        .await
        .unwrap();
    rows
}

/// What one bounded write gave: the stopped statement, the later
/// statement, the commit, and the write itself.
struct Outcome {
    stopped: anyhow::Error,
    later: anyhow::Result<u64>,
    commit: anyhow::Result<()>,
    write: anyhow::Result<()>,
}

/// One write that keeps the row 1, bounds its work to [`BOUND`], runs
/// `past`, keeps the row 2, and commits.
async fn bounded_write<F>(file: &File, past: F) -> Outcome
where
    F: for<'t> FnOnce(&'t mut Transaction) -> SqlFuture<'t, ()> + Send + 'static,
{
    let mut seen = None;
    let slot = &mut seen;
    let write = file
        .database
        .write(Box::new(move |mut transaction| {
            Box::pin(async move {
                transaction
                    .execute("INSERT INTO kept (n) VALUES (1)", Vec::new())
                    .await?;
                transaction.bound_work(BOUND).await?;
                let stopped = past(&mut transaction).await;
                let later = transaction
                    .execute("INSERT INTO kept (n) VALUES (2)", Vec::new())
                    .await;
                let commit = transaction.commit().await;
                let failed = commit.as_ref().err().map(|error| format!("{error:#}"));
                *slot = Some((stopped, later, commit));
                failed.map_or(Ok(()), |error| Err(anyhow::anyhow!(error)))
            })
        }))
        .await;
    let (stopped, later, commit) = seen.expect("the write ran its work");
    Outcome {
        stopped: stopped.expect_err("the statement past the bound ran to its end"),
        later,
        commit,
        write,
    }
}

fn assert_over_budget(error: &anyhow::Error) {
    assert!(
        error.downcast_ref::<OverBudget>().is_some(),
        "not OverBudget: {error:#}"
    );
}

/// A statement that reads and that the file stops keeps the transaction:
/// the earlier and the later rows are kept, and the commit succeeds.
#[tokio::test]
async fn a_stopped_read_keeps_the_transaction() {
    let file = File::default();
    tables(&file).await;
    let outcome = bounded_write(&file, |transaction| {
        Box::pin(async move { transaction.fetch(READ_PAST, Vec::new()).await.map(drop) })
    })
    .await;
    assert_over_budget(&outcome.stopped);
    assert_eq!(outcome.later.unwrap(), 1);
    outcome.commit.unwrap();
    outcome.write.unwrap();
    assert_eq!(
        kept(&file).await,
        vec![vec![SqlValue::Integer(1)], vec![SqlValue::Integer(2)]]
    );
}

/// A statement that writes and that the file stops ends the transaction:
/// `SQLite` rolls it back, so the earlier row is lost. The later statement
/// runs outside any transaction, so its row is kept, and the commit fails.
#[tokio::test]
async fn a_stopped_write_ends_the_transaction() {
    let file = File::default();
    tables(&file).await;
    let outcome = bounded_write(&file, |transaction| {
        Box::pin(async move { transaction.execute(WRITE_PAST, Vec::new()).await.map(drop) })
    })
    .await;
    assert_over_budget(&outcome.stopped);
    assert_eq!(outcome.later.unwrap(), 1);
    let commit = outcome.commit.unwrap_err();
    assert!(
        format!("{commit:#}").contains("no transaction is active"),
        "{commit:#}"
    );
    outcome.write.unwrap_err();
    assert_eq!(kept(&file).await, vec![vec![SqlValue::Integer(2)]]);
}

/// A statement in an attempt that the file stops ends the transaction as
/// a statement outside an attempt does, and the outer error of the
/// attempt is `OverBudget`.
#[tokio::test]
async fn a_stopped_attempt_gives_over_budget() {
    let file = File::default();
    tables(&file).await;
    let outcome = bounded_write(&file, |transaction| {
        Box::pin(async move {
            let statements = [Statement {
                sql: WRITE_PAST.to_owned(),
                values: Vec::new(),
            }];
            transaction.attempt(&statements).await?.map(drop)
        })
    })
    .await;
    assert_over_budget(&outcome.stopped);
    outcome.commit.unwrap_err();
    outcome.write.unwrap_err();
    assert_eq!(kept(&file).await, vec![vec![SqlValue::Integer(2)]]);
}

/// The file counts the steps of each statement text in whole intervals of
/// 1,000 engine steps, over all the runs of that text on the connection.
/// With a bound of no steps, a hundred distinct statements of fewer steps
/// each give their rows, and one such statement that runs again and again
/// is stopped once its runs together pass one interval.
#[tokio::test]
async fn the_file_counts_each_statement_text_at_whole_intervals() {
    let file = File::default();
    tables(&file).await;
    let mut seen = None;
    let slot = &mut seen;
    file.database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                transaction.bound_work(0).await?;
                let mut distinct = Vec::new();
                for n in 1..=100 {
                    let sql = format!("SELECT COUNT(*) FROM wide WHERE rowid <= {n}");
                    distinct.push(transaction.fetch(&sql, Vec::new()).await?);
                }
                let mut repeated = None;
                for run in 0..100 {
                    let sql = "SELECT COUNT(*) FROM wide WHERE rowid <= 20";
                    if let Err(error) = transaction.fetch(sql, Vec::new()).await {
                        repeated = Some((run, error));
                        break;
                    }
                }
                *slot = Some((distinct, repeated));
                Ok(())
            })
        }))
        .await
        .unwrap();
    let (distinct, repeated) = seen.expect("the read ran its work");
    let expected: Vec<Vec<SqlRow>> = (1..=100)
        .map(|n| vec![vec![SqlValue::Integer(n)]])
        .collect();
    assert_eq!(distinct, expected);
    let (run, error) = repeated.expect("the repeated statement was not stopped");
    assert!(run > 0, "the first run of the statement was stopped");
    assert_over_budget(&error);
}
