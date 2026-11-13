//! The range of the integers that the port binds and reads: magnitude at
//! most [`LARGEST_EXACT_INTEGER`], 2^53 − 1.

use super::{fetch, statement, write, Harness};
use crate::read_database::{OutOfRange, ReadDatabase, SqlRow, SqlValue};
use provenance_core::model::exact_integer::LARGEST_EXACT_INTEGER;

/// The error of one statement in one read.
async fn read_error(database: &dyn ReadDatabase, sql: &'static str) -> anyhow::Error {
    let mut answer = None;
    let slot = &mut answer;
    database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                *slot = Some(transaction.fetch(sql, Vec::new()).await);
                Ok(())
            })
        }))
        .await
        .unwrap();
    answer
        .expect("the read ran its work")
        .expect_err("the read gave an integer past the range")
}

/// A bound integer past the range is refused with [`OutOfRange`] before the
/// statement runs, and the write keeps nothing; a row that holds an integer
/// past the range is refused with [`OutOfRange`].
///
/// The statement that binds the value names a table that does not exist,
/// so an adapter that runs it before the check gives another error.
pub async fn an_integer_past_the_range_is_refused_and_nothing_is_written<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![statement("CREATE TABLE kept (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
    for past in [
        LARGEST_EXACT_INTEGER + 1,
        -LARGEST_EXACT_INTEGER - 1,
        i64::MAX,
        i64::MIN,
    ] {
        let refused = write(
            database,
            vec![
                statement("INSERT INTO kept (n) VALUES (1)", Vec::new()),
                statement(
                    "INSERT INTO absent_table (n) VALUES (?)",
                    vec![SqlValue::Integer(past)],
                ),
            ],
        )
        .await
        .unwrap_err();
        assert_eq!(
            refused.downcast_ref::<OutOfRange>().map(|error| error.0),
            Some(past),
            "{refused:#}"
        );
    }
    assert_eq!(
        fetch(database, "SELECT n FROM kept").await,
        Vec::<SqlRow>::new()
    );
    for (sql, past) in [
        ("SELECT 9007199254740991 + 1", LARGEST_EXACT_INTEGER + 1),
        ("SELECT -9007199254740991 - 1", -LARGEST_EXACT_INTEGER - 1),
    ] {
        let refused = read_error(database, sql).await;
        assert_eq!(
            refused.downcast_ref::<OutOfRange>().map(|error| error.0),
            Some(past),
            "{refused:#}"
        );
    }
}
