//! The contract that every read database adapter satisfies.
//!
//! Each case takes a fresh harness, whose database holds no table. The
//! cases use only the port: the SQL that they run is the SQL that the
//! projection runs, so a store that passes them can hold the projection.

use super::{ReadDatabase, SqlRow, SqlValue, Statement};

/// One adapter under test.
pub trait Harness {
    /// The database under test. It holds no table at the start.
    fn database(&self) -> &dyn ReadDatabase;
    /// A second adapter over the same store, as a later process opens it.
    fn reopen(&self) -> Box<dyn ReadDatabase>;
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
                for statement in &statements {
                    transaction
                        .execute(&statement.sql, &statement.values)
                        .await?;
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
                *slot = transaction.fetch(sql, &[]).await?;
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

/// A write keeps each value with its storage class, and a later read gives
/// it back unchanged.
pub async fn a_write_keeps_each_value_class<H: Harness>(harness: H) {
    let database = harness.database();
    let values = vec![
        SqlValue::Null,
        SqlValue::Integer(i64::MIN),
        SqlValue::Integer(i64::MAX),
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
    assert!(failed.is_err());
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
                    .execute("INSERT INTO kept (n) VALUES (1)", &[])
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

/// Every row that a read gets is from the state that was committed when
/// the read started, also when a write commits while the read runs.
pub async fn a_read_answers_from_one_committed_state<H: Harness>(harness: H) {
    let database = harness.database();
    write(
        database,
        vec![
            statement("CREATE TABLE pinned (n INTEGER)", Vec::new()),
            statement("INSERT INTO pinned (n) VALUES (1)", Vec::new()),
        ],
    )
    .await
    .unwrap();
    let mut seen = Vec::new();
    let slot = &mut seen;
    database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                let sql = "SELECT n FROM pinned ORDER BY n";
                let before = transaction.fetch(sql, &[]).await?;
                write(
                    database,
                    vec![statement("INSERT INTO pinned (n) VALUES (2)", Vec::new())],
                )
                .await?;
                let after = transaction.fetch(sql, &[]).await?;
                *slot = vec![integers(&before), integers(&after)];
                Ok(())
            })
        }))
        .await
        .unwrap();
    assert_eq!(seen, vec![vec![1], vec![1]]);
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM pinned ORDER BY n").await),
        vec![1, 2]
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
                    .execute("INSERT INTO attempted (n) VALUES (1)", &[])
                    .await?;
                let failed = transaction.attempt(&[insert(2), insert(1)]).await?;
                let kept = transaction.attempt(&[insert(3), insert(4)]).await?;
                transaction
                    .execute("INSERT INTO attempted (n) VALUES (5)", &[])
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
