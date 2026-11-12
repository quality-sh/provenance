//! The snapshot handles over the projection tables.
//!
//! The cases run each handle in one read of the port, so a store that
//! passes them gives the handles the rows that the file gives. They do not
//! cover the discussion reads that still run `SQLx` on the file. Of the
//! work bound of a page, they hold only that a statement that the adapter
//! stops with [`OverBudget`] gives the page budget refusal (see also
//! [`super::limits`]).

use super::{statement, write, Harness, Statement};
use crate::operations::reader::{page_error, Position, ReadSnapshot};
use crate::read_database::{OverBudget, SqlFuture, SqlRow, SqlTransaction, SqlValue, Transaction};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::{Requirement, ScopeId, StableId};

const SCHEMA: &str = include_str!("../../../current_cache.sql");

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

/// A list handle gives the ids of a table in id order, at most its
/// limit of them, and its records.
pub async fn the_list_handles_read_the_projection<H: Harness + Send + Sync>(harness: H) {
    let answer = answer(&harness, |snapshot| {
        Box::pin(async move {
            let table = snapshot.table::<Requirement>();
            let page = table.search_ids("", 10, false).await?;
            let first = table.search_ids("", 1, false).await?;
            let wanted = [StableId::new("req_root")?, StableId::new("req_none")?];
            let found = table.by_ids(&wanted).await?;
            let record = table.record(&StableId::new("req_child")?).await?;
            Ok(format!(
                "{page:?} {first:?} {:?} {:?} {}",
                ids(&found),
                record.map(|record| record.refines),
                table.count().await?
            ))
        })
    })
    .await;
    assert_eq!(
        answer,
        r#"["req_child", "req_root"] ["req_child"] ["req_root"] Some([StableId("req_root")]) 2"#
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

/// A transaction that stops each statement after
/// [`SqlTransaction::bound_work`] with [`OverBudget`], as an adapter does
/// when a statement passes the bound.
struct Stopping {
    inner: Transaction,
    bounded: bool,
}

impl SqlTransaction for Stopping {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        if self.bounded {
            Box::pin(async {
                Err(OverBudget(anyhow::anyhow!("the store stopped the statement")).into())
            })
        } else {
            self.inner.fetch(sql, values)
        }
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

    fn bound_work(&mut self, _steps: u64) -> SqlFuture<'_, ()> {
        self.bounded = true;
        Box::pin(async { Ok(()) })
    }

    fn max_list_values(&self) -> usize {
        self.inner.max_list_values()
    }
}

/// A page statement that the adapter stops for the work bound, with
/// [`OverBudget`], gives the page budget refusal.
pub async fn a_stopped_page_statement_is_the_page_budget_refusal<H: Harness + Send + Sync>(
    harness: H,
) {
    let database = harness.database();
    write(database, projection()).await.unwrap();
    let mut refused = None;
    let slot = &mut refused;
    database
        .read(Box::new(move |transaction| {
            Box::pin(async move {
                let stopping = Box::new(Stopping {
                    inner: transaction,
                    bounded: false,
                });
                let scope = ScopeId::new("default")?;
                let snapshot = ReadSnapshot::open(stopping, &scope)
                    .await?
                    .expect("a revision");
                snapshot.bound_page_work().await?;
                let page = snapshot
                    .table::<Requirement>()
                    .search_ids("", 10, false)
                    .await
                    .map_err(page_error);
                *slot = Some(page.map(|ids| format!("{ids:?}")));
                Ok(())
            })
        }))
        .await
        .unwrap();
    let failed = refused.expect("the read ran").unwrap_err();
    assert!(
        matches!(
            failed.downcast_ref::<ReadFailure>(),
            Some(ReadFailure::PageBudgetExceeded)
        ),
        "{failed:#}"
    );
}
