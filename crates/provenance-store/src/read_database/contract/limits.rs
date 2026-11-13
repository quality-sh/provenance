//! The limits that the readers put on a read: the work bound of a page,
//! the `LIMIT` of a page statement, and the size of a bound list.
//!
//! For the work bound, the contract holds that a bound that the work does
//! not reach changes nothing, and that a statement far past the bound
//! gives [`OverBudget`] and no rows. It does not check whether the adapter
//! stops the statement while it runs or reports it when it ends; the file
//! adapter stops it, and its own tests hold that.

use super::{integers, statement, write, Harness};
use crate::read_database::{OverBudget, ReadDatabase, SqlRow, SqlValue, Transaction, WorkUnit};

/// A query whose work grows with `n`: it counts from 1 to `n`. The
/// tests of the work bound share it.
#[doc(hidden)]
pub const COUNT_TO: &str = "WITH RECURSIVE count_to (n) AS (SELECT 1 UNION ALL \
                        SELECT n + 1 FROM count_to WHERE n < ?) SELECT COUNT(*) FROM count_to";

/// Writes a table `numbers` with the integers 1 to 5.
async fn numbers(database: &dyn ReadDatabase) {
    write(
        database,
        vec![
            statement("CREATE TABLE numbers (n INTEGER)", Vec::new()),
            statement(
                "INSERT INTO numbers (n) VALUES (1), (2), (3), (4), (5)",
                Vec::new(),
            ),
        ],
    )
    .await
    .unwrap();
}

/// Runs `work` in one read and gives its answer.
async fn read<T, F>(database: &dyn ReadDatabase, work: F) -> anyhow::Result<T>
where
    T: Send,
    F: for<'t> FnOnce(
            &'t mut Transaction,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 't>,
        > + Send,
{
    let mut answer = None;
    let slot = &mut answer;
    database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                *slot = Some(work(&mut transaction).await);
                Ok(())
            })
        }))
        .await?;
    answer.expect("the read ran its work")
}

fn count(rows: &[SqlRow]) -> i64 {
    integers(rows)[0]
}

/// After `bound_work`, the statements that stay within the bound give
/// their rows.
pub async fn a_bounded_read_runs_the_statements_within_its_bound<H: Harness>(harness: H) {
    let database = harness.database();
    numbers(database).await;
    let rows = read(database, |transaction| {
        Box::pin(async move {
            transaction.bound_work(10_000_000).await?;
            let small = transaction
                .fetch(COUNT_TO, vec![SqlValue::Integer(100)])
                .await?;
            let all = transaction
                .fetch("SELECT n FROM numbers ORDER BY n", Vec::new())
                .await?;
            Ok((count(&small), integers(&all)))
        })
    })
    .await
    .unwrap();
    assert_eq!(rows, (100, vec![1, 2, 3, 4, 5]));
}

/// A statement gives at most the rows that its `LIMIT` names, bound or
/// written in the text.
pub async fn a_limit_bounds_the_rows<H: Harness>(harness: H) {
    let database = harness.database();
    numbers(database).await;
    let rows = read(database, |transaction| {
        Box::pin(async move {
            let bound = transaction
                .fetch(
                    "SELECT n FROM numbers WHERE n > ? ORDER BY n LIMIT ?",
                    vec![SqlValue::Integer(1), SqlValue::Integer(2)],
                )
                .await?;
            let written = transaction
                .fetch("SELECT n FROM numbers ORDER BY n DESC LIMIT 1", Vec::new())
                .await?;
            Ok((integers(&bound), integers(&written)))
        })
    })
    .await
    .unwrap();
    assert_eq!(rows, (vec![2, 3], vec![5]));
}

/// A list of the most values that a reader binds runs with the two values
/// that a reader binds beside it.
pub async fn a_list_of_the_most_values_runs<H: Harness>(harness: H) {
    let database = harness.database();
    numbers(database).await;
    let most = database.max_bound_values();
    let found = read(database, move |transaction| {
        Box::pin(async move {
            let list = transaction.max_list_values();
            assert!(list >= 1, "a list of no values");
            assert!(list + 2 <= most, "{list} values and two more pass {most}");
            let marks = vec!["?"; list].join(", ");
            let mut values = vec![SqlValue::Integer(0)];
            values.extend((1..=list).map(|n| SqlValue::Integer(i64::try_from(n).unwrap())));
            values.push(SqlValue::Integer(2));
            let rows = transaction
                .fetch(
                    &format!("SELECT COUNT(*) FROM numbers WHERE n > ? AND n IN ({marks}) LIMIT ?"),
                    values,
                )
                .await?;
            Ok(count(&rows))
        })
    })
    .await
    .unwrap();
    assert_eq!(found, 5);
}

/// A bound in each unit that [`FAR_PAST`] passes a hundred times or more,
/// however the store plans it.
const fn small_bound(unit: WorkUnit) -> u64 {
    match unit {
        WorkUnit::EngineSteps => 10_000,
        WorkUnit::RowsRead => 10,
    }
}

/// A statement over each of the million pairs of the 1,000 rows of
/// `wide`. A store reads each row of `wide` at least once, so it reads at
/// least 1,000 rows, also when it reads `wide` once for both sides. The
/// sum needs a value of each pair, so the store runs more than 1,000,000
/// steps.
const FAR_PAST: &str = "SELECT SUM((a.n * 7 + b.n * 13) % 101) FROM wide AS a, wide AS b";

/// After `bound_work`, a statement far past the bound gives [`OverBudget`].
///
/// The statement gives no rows. The transaction stays open: a later
/// statement in it gives its rows or [`OverBudget`], and the read ends
/// without an error.
pub async fn a_statement_far_past_the_bound_gives_over_budget<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![
            statement("CREATE TABLE wide (n INTEGER)", Vec::new()),
            statement(
                "INSERT INTO wide (n) WITH RECURSIVE count_to (n) AS (SELECT 1 UNION ALL \
                 SELECT n + 1 FROM count_to WHERE n < 1000) SELECT n FROM count_to",
                Vec::new(),
            ),
        ],
    )
    .await
    .unwrap();
    let (far, later) = read(database, |transaction| {
        Box::pin(async move {
            let bound = small_bound(transaction.work_unit());
            transaction.bound_work(bound).await?;
            let far = transaction.fetch(FAR_PAST, Vec::new()).await;
            let later = transaction.fetch("SELECT 1", Vec::new()).await;
            Ok((far, later))
        })
    })
    .await
    .unwrap();
    let stopped = far.expect_err("the statement past the bound gave its rows");
    assert!(
        stopped.downcast_ref::<OverBudget>().is_some(),
        "not OverBudget: {stopped:#}"
    );
    match later {
        Ok(rows) => assert_eq!(rows, vec![vec![SqlValue::Integer(1)]]),
        Err(error) => assert!(
            error.downcast_ref::<OverBudget>().is_some(),
            "the transaction failed after OverBudget: {error:#}"
        ),
    }
}
