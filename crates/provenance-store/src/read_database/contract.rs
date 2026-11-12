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

/// The snapshot handles over the projection tables. The cases run each
/// handle in one read of the port, so a store that passes them answers
/// the reads that the operations make.
#[cfg(feature = "host")]
pub mod handles {
    use super::{statement, write, Harness, Statement};
    use crate::operations::reader::{Position, ReadSnapshot};
    use provenance_core::{Requirement, ScopeId, StableId};

    const SCHEMA: &str = include_str!("../../current_cache.sql");

    /// The projection schema, then one revision, two requirements (the
    /// child refines the root), and a thread on the root with one message.
    fn projection() -> Vec<Statement> {
        let mut statements: Vec<Statement> = SCHEMA
            .split(';')
            .filter(|sql| !sql.trim().is_empty())
            .map(|sql| statement(sql, Vec::new()))
            .collect();
        let requirement = "INSERT INTO requirements (scope_id, id, statement, status, \
            source_refs, refines, depends_on, supersedes, search_text) \
            VALUES ('default', ?, ?, 'active', '[]', ?, '[]', '[]', '')";
        for sql in [
            "INSERT INTO projection_instance (only_row, instance_id) VALUES (1, 'instance')",
            "INSERT INTO projection_revision (serial, digest) VALUES (1, 'sha256:revision')",
            "INSERT INTO relations VALUES \
             ('default', 'requirement', 'req_child', 'refines', 'requirement', 'req_root')",
            "INSERT INTO threads VALUES \
             ('default', 'thread_one', 'requirement', 'req_root', 'active', 5, '{}')",
            "INSERT INTO messages VALUES \
             ('default', 'message_one', 'thread_one', 'user', 'Hello', 7, '{}')",
        ] {
            statements.push(statement(sql, Vec::new()));
        }
        for (id, refines) in [("req_root", "[]"), ("req_child", r#"["req_root"]"#)] {
            let values = vec![id.into(), format!("{id} statement").into(), refines.into()];
            statements.push(statement(requirement, values));
        }
        statements
    }

    /// Writes the projection, then runs `read` over a snapshot of it and
    /// gives its answer as text.
    async fn answer<H, F>(harness: &H, read: F) -> String
    where
        H: Harness + Sync,
        F: for<'s> FnOnce(
                &'s ReadSnapshot,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 's>,
            > + Send,
    {
        let database = harness.database();
        write(database, projection()).await.unwrap();
        let mut answer = String::new();
        let slot = &mut answer;
        database
            .read(Box::new(move |transaction| {
                Box::pin(async move {
                    let scope = ScopeId::new("default")?;
                    let snapshot = ReadSnapshot::open(transaction, &scope)
                        .await?
                        .expect("a revision");
                    *slot = read(&snapshot).await?;
                    Ok(())
                })
            }))
            .await
            .unwrap();
        answer
    }

    fn ids(records: &[Requirement]) -> Vec<&str> {
        records.iter().map(|record| record.id.as_str()).collect()
    }

    /// A list handle gives the ids of a table in id order, and its records.
    pub async fn the_list_handles_read_the_projection<H: Harness + Send + Sync>(harness: H) {
        let answer = answer(&harness, |snapshot| {
            Box::pin(async move {
                let table = snapshot.table::<Requirement>();
                let page = table.search_ids("", 10, false).await?;
                let wanted = [StableId::new("req_root")?, StableId::new("req_none")?];
                let found = table.by_ids(&wanted).await?;
                let record = table.record(&StableId::new("req_child")?).await?;
                Ok(format!(
                    "{page:?} {:?} {:?} {}",
                    ids(&found),
                    record.map(|record| record.refines),
                    table.count().await?
                ))
            })
        })
        .await;
        assert_eq!(
            answer,
            r#"["req_child", "req_root"] ["req_root"] Some([StableId("req_root")]) 2"#
        );
    }

    /// The document handle gives the keys of a requirement's document:
    /// its members, then its threads, then their messages.
    pub async fn the_document_handle_reads_the_projection<H: Harness + Send + Sync>(harness: H) {
        let answer = answer(&harness, |snapshot| {
            Box::pin(async move {
                let keys = snapshot
                    .document_keys("req_root", &Position::default(), 50, false, &[])
                    .await?;
                Ok(keys
                    .iter()
                    .map(|(kind, key)| {
                        format!(
                            "{kind} {} {} {} {}",
                            key.stage, key.rank, key.counter, key.id
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; "))
            })
        })
        .await;
        assert_eq!(
            answer,
            "requirement 0 0 0 req_root; requirement 0 2 0 req_child; \
             thread 2 1 5 thread_one; message 3 1 7 message_one"
        );
    }

    /// The discussion handle gives each thread and one thread by id.
    pub async fn the_discussion_handle_reads_the_projection<H: Harness + Send + Sync>(harness: H) {
        let answer = answer(&harness, |snapshot| {
            Box::pin(async move {
                let all = snapshot.threads().await?;
                let one = snapshot.page_thread("thread_one").await?;
                let none = snapshot.page_thread("thread_none").await?;
                Ok(format!("{all:?} {} {none:?}", one == all.first().cloned()))
            })
        })
        .await;
        assert_eq!(
            answer,
            "[Thread { scope_id: ScopeId(\"default\"), id: StableId(\"thread_one\"), \
             parent: ThreadParent { node_type: Requirement, node_id: StableId(\"req_root\") }, \
             status: Active, created_at: 5 }] true None"
        );
    }

    /// The message handle gives each message and one message by id.
    pub async fn the_message_handle_reads_the_projection<H: Harness + Send + Sync>(harness: H) {
        let answer = answer(&harness, |snapshot| {
            Box::pin(async move {
                let all = snapshot.messages().await?;
                let one = snapshot.page_message("message_one").await?;
                Ok(format!("{all:?} {}", one == all.first().cloned()))
            })
        })
        .await;
        assert_eq!(
            answer,
            "[Message { scope_id: ScopeId(\"default\"), id: StableId(\"message_one\"), \
             thread_id: StableId(\"thread_one\"), role: User, body: \"Hello\", created_at: 7 }] true"
        );
    }
}
