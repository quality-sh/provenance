//! The contract that every read database adapter satisfies.
//!
//! Each case takes a fresh harness, whose database nothing has written.
//! The cases use only the port: the SQL that they run is the SQL that the
//! projection runs, so a store that passes them can hold the projection.
//! No case runs one port call inside the work of another, and no work
//! waits on anything but its own transaction.

use super::{
    is_missing_table, ReadDatabase, SqlFuture, SqlRow, SqlValue, Statement, Transaction, Unopened,
};
use provenance_core::model::exact_integer::LARGEST_EXACT_INTEGER;

pub mod handles;
#[cfg(any(test, feature = "test-fixture"))]
mod hooked;
pub mod limits;
#[cfg(any(test, feature = "test-fixture"))]
mod other;
#[cfg(feature = "host")]
pub mod ownership;
#[cfg(feature = "host")]
pub mod projection;
pub mod range;
pub mod sequence;

#[cfg(any(test, feature = "test-fixture"))]
pub use hooked::{Hook, Hooked, HookedTransaction};
#[cfg(any(test, feature = "test-fixture"))]
pub use other::{Other, OtherTransaction};

/// One adapter under test.
pub trait Harness {
    /// The database under test. Nothing has written it at the start.
    fn database(&self) -> &dyn ReadDatabase;
    /// A second adapter over the same store, as a later process opens it.
    fn reopen(&self) -> Box<dyn ReadDatabase>;
    /// Whether a write can commit while a read of the store is open.
    ///
    /// This answer changes one check of one case,
    /// [`sequence::a_read_answers_from_one_committed_state`]: with `true`,
    /// the write that the case starts beside the read must commit before
    /// the read ends. With `false`, the case does not check when that write
    /// commits. A store whose read runs to its end in one call says no, and
    /// then the write runs after the read.
    ///
    /// The rest of that case runs with either answer: the write starts
    /// after the first statement of the read, every row that the read gets
    /// must be from the state committed before it, the write must succeed,
    /// and a later read must find the rows of both writes. So a store whose
    /// read sees a write that commits while it is open fails the case with
    /// either answer. No other case reads this answer.
    fn reads_interleave(&self) -> bool {
        true
    }
    /// Makes the commit of the write `transaction` fail, so that the write
    /// keeps nothing.
    ///
    /// The work commits after this call, as every write work does. A store
    /// that commits in the work fails that commit; a store that makes its
    /// commit durable after the work fails it there.
    ///
    /// This call itself must succeed: when it returns an error, the commit
    /// case fails, because the write then fails in a step of its work and
    /// not at its commit.
    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()>;
}

fn statement(sql: &str, values: Vec<SqlValue>) -> Statement {
    Statement {
        sql: sql.to_owned(),
        values,
    }
}

/// Runs `statements` in one write, in order.
async fn write(database: &dyn ReadDatabase, statements: Vec<Statement>) -> anyhow::Result<()> {
    database
        .write(Box::new(move |mut transaction| {
            Box::pin(async move {
                for Statement { sql, values } in statements {
                    transaction.execute(&sql, values).await?;
                }
                transaction.commit().await
            })
        }))
        .await
}

/// The rows of one statement, in one read.
async fn fetch(database: &dyn ReadDatabase, sql: &'static str) -> Vec<SqlRow> {
    let mut rows = Vec::new();
    let slot = &mut rows;
    database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                *slot = transaction.fetch(sql, Vec::new()).await?;
                Ok(())
            })
        }))
        .await
        .unwrap();
    rows
}

fn integers(rows: &[SqlRow]) -> Vec<i64> {
    rows.iter()
        .map(|row| match row.as_slice() {
            [SqlValue::Integer(value)] => *value,
            other => panic!("not one integer: {other:?}"),
        })
        .collect()
}

/// The error of a work that fails on purpose.
#[derive(Debug, thiserror::Error)]
#[error("the work failed after its commit")]
struct WorkFailed;

/// A read of a database that nothing has written fails with [`Unopened`]
/// before its work starts, and makes nothing: a second read fails in the
/// same way.
pub async fn a_read_of_an_absent_database_refuses_before_its_work<H: Harness + Sync>(harness: &H) {
    for attempt in 0..2 {
        let mut started = false;
        let slot = &mut started;
        let failed = harness
            .database()
            .read(Box::new(move |_| {
                Box::pin(async move {
                    *slot = true;
                    Ok(())
                })
            }))
            .await
            .unwrap_err();
        assert!(!started, "read {attempt}");
        assert!(
            failed.downcast_ref::<Unopened>().is_some(),
            "read {attempt}: {failed:#}"
        );
    }
}

/// A statement that names an absent table fails with an error that
/// [`is_missing_table`] knows, and no other failure does.
///
/// The engine reads a database with no revision table as one that holds
/// no revision.
pub async fn a_missing_table_is_reported_as_missing<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement("CREATE TABLE present (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
    let mut failures = Vec::new();
    let slot = &mut failures;
    database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                for sql in [
                    "SELECT n FROM absent_table",
                    "SELECT absent_column FROM present",
                ] {
                    let failed = transaction.fetch(sql, Vec::new()).await.unwrap_err();
                    slot.push(is_missing_table(&failed));
                }
                Ok(())
            })
        }))
        .await
        .unwrap();
    assert_eq!(failures, vec![true, false]);
}

/// A write keeps each value with its storage class, and a later read gives
/// it back unchanged. The integers are the two ends of the range of the
/// port.
pub async fn a_write_keeps_each_value_class<H: Harness>(harness: H) {
    let database = harness.database();
    let values = vec![
        SqlValue::Null,
        SqlValue::Integer(-LARGEST_EXACT_INTEGER),
        SqlValue::Integer(LARGEST_EXACT_INTEGER),
        SqlValue::Real(-0.5),
        SqlValue::Text(String::new()),
        SqlValue::Text("Ünïcode \u{1F600} 'quoted'".to_owned()),
        SqlValue::Blob(Vec::new()),
        SqlValue::Blob(vec![0, 0xff, 0x80]),
    ];
    let mut statements = vec![statement(
        "CREATE TABLE value_classes (n INTEGER PRIMARY KEY, v)",
        Vec::new(),
    )];
    for (n, value) in (0_i64..).zip(&values) {
        statements.push(statement(
            "INSERT INTO value_classes (n, v) VALUES (?, ?)",
            vec![SqlValue::Integer(n), value.clone()],
        ));
    }
    write(database, statements).await.unwrap();
    let rows = fetch(database, "SELECT v FROM value_classes ORDER BY n").await;
    let read: Vec<SqlValue> = rows.into_iter().flatten().collect();
    assert_eq!(read, values);
}

/// A write whose work fails keeps none of its statements.
pub async fn a_failed_write_keeps_nothing<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement("CREATE TABLE kept (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
    let failed = write(
        database,
        vec![
            statement("INSERT INTO kept (n) VALUES (1)", Vec::new()),
            statement("INSERT INTO absent_table (n) VALUES (2)", Vec::new()),
        ],
    )
    .await;
    let failed = failed.unwrap_err();
    assert!(is_missing_table(&failed), "{failed:#}");
    assert_eq!(
        fetch(database, "SELECT n FROM kept").await,
        Vec::<SqlRow>::new()
    );
}

/// A write whose work commits and then fails keeps what it committed, and
/// gives the error of the work. Catch-up relies on it: a pass that fails
/// after its commit has still published its revision.
pub async fn a_write_that_fails_after_its_commit_keeps_the_commit<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement("CREATE TABLE kept (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
    let failed = database
        .write(Box::new(|mut transaction| {
            Box::pin(async move {
                transaction
                    .execute("INSERT INTO kept (n) VALUES (1)", Vec::new())
                    .await?;
                transaction.commit().await?;
                Err(WorkFailed.into())
            })
        }))
        .await
        .unwrap_err();
    assert!(failed.downcast_ref::<WorkFailed>().is_some(), "{failed:#}");
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM kept").await),
        vec![1]
    );
}

/// A read keeps nothing that its work writes.
pub async fn a_read_keeps_no_writes<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement("CREATE TABLE kept (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
    database
        .read(Box::new(|mut transaction| {
            Box::pin(async move {
                // A store may refuse the statement; either way it keeps
                // nothing.
                let _ = transaction
                    .execute("INSERT INTO kept (n) VALUES (1)", Vec::new())
                    .await;
                Ok(())
            })
        }))
        .await
        .unwrap();
    assert_eq!(
        fetch(database, "SELECT n FROM kept").await,
        Vec::<SqlRow>::new()
    );
}

/// A write whose work ends without the commit keeps none of its
/// statements.
pub async fn a_write_without_its_commit_keeps_nothing<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement("CREATE TABLE kept (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
    database
        .write(Box::new(|mut transaction| {
            Box::pin(async move {
                transaction
                    .execute("INSERT INTO kept (n) VALUES (1)", Vec::new())
                    .await?;
                Ok(())
            })
        }))
        .await
        .unwrap();
    assert_eq!(
        fetch(database, "SELECT n FROM kept").await,
        Vec::<SqlRow>::new()
    );
}

/// An attempt keeps all of its statements or none of them, and the
/// transaction continues after an attempt that failed.
pub async fn an_attempt_keeps_all_or_none<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement(
            "CREATE TABLE attempted (n INTEGER UNIQUE)",
            Vec::new(),
        )],
    )
    .await
    .unwrap();
    let mut failures = Vec::new();
    let slot = &mut failures;
    database
        .write(Box::new(move |mut transaction| {
            Box::pin(async move {
                let insert = |n: i64| {
                    statement(
                        "INSERT INTO attempted (n) VALUES (?)",
                        vec![SqlValue::Integer(n)],
                    )
                };
                transaction
                    .execute("INSERT INTO attempted (n) VALUES (1)", Vec::new())
                    .await?;
                let failed = transaction.attempt(&[insert(2), insert(1)]).await?;
                let kept = transaction.attempt(&[insert(3), insert(4)]).await?;
                transaction
                    .execute("INSERT INTO attempted (n) VALUES (5)", Vec::new())
                    .await?;
                *slot = vec![failed.is_err(), kept.is_err()];
                transaction.commit().await
            })
        }))
        .await
        .unwrap();
    assert_eq!(failures, vec![true, false]);
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM attempted ORDER BY n").await),
        vec![1, 3, 4, 5]
    );
}

/// The statement forms that the projection uses run: a recursive query,
/// the JSON functions, `instr`, and one insert that binds the maximum
/// number of values.
pub async fn the_projection_statement_forms_run<H: Harness>(harness: H) {
    let database = harness.database();
    let most = database.max_bound_values();
    let rows = most / 3;
    let values: Vec<SqlValue> = (0..rows * 3)
        .map(|n| SqlValue::Integer(i64::try_from(n).unwrap()))
        .collect();
    let placeholders = vec!["(?, ?, ?)"; rows].join(", ");
    write(
        database,
        vec![
            statement(
                "CREATE TABLE wide (a INTEGER, b INTEGER, c INTEGER)",
                Vec::new(),
            ),
            statement(
                &format!("INSERT INTO wide (a, b, c) VALUES {placeholders}"),
                values,
            ),
            statement("CREATE TABLE doc (body TEXT)", Vec::new()),
            statement(
                "INSERT INTO doc (body) VALUES (?)",
                vec![SqlValue::from(r#"{"words":["alpha","beta"],"n":7}"#)],
            ),
            statement(
                "CREATE INDEX doc_n ON doc (json_extract(body, '$.n'))",
                Vec::new(),
            ),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        fetch(database, "SELECT COUNT(*) FROM wide").await,
        vec![vec![SqlValue::Integer(i64::try_from(rows).unwrap())]]
    );
    assert_eq!(
        integers(
            &fetch(
                database,
                "WITH RECURSIVE count_to (n) AS (SELECT 1 UNION ALL \
                 SELECT n + 1 FROM count_to WHERE n < 3) SELECT n FROM count_to",
            )
            .await
        ),
        vec![1, 2, 3]
    );
    assert_eq!(
        fetch(
            database,
            "SELECT words.value, instr(words.value, 'et') FROM doc, \
             json_each(json_extract(doc.body, '$.words')) AS words ORDER BY words.key",
        )
        .await,
        vec![
            vec![SqlValue::from("alpha"), SqlValue::Integer(0)],
            vec![SqlValue::from("beta"), SqlValue::Integer(2)],
        ]
    );
}

/// The cursor key is 32 bytes, and it stays the same for a later adapter
/// over the same store.
pub fn the_cursor_key_stays_the_same<H: Harness>(harness: &H) {
    let first = harness.database().cursor_key().unwrap();
    assert_eq!(harness.database().cursor_key().unwrap(), first);
    assert_eq!(harness.reopen().cursor_key().unwrap(), first);
}
