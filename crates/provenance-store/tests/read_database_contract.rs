//! The read database contract on the file adapter.
//!
//! No Rule: the plan proposes `rule_read_database_adapters_meet_contract`
//! for the owner's review. Until it is approved, these tests hold the
//! contract while the projection moves behind the port.

use camino::Utf8PathBuf;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::stamp::stored_revision;
use provenance_store::read_database::contract::{self, Harness, Other};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::read_database::{
    OverBudget, ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement,
    Transaction, WriteWork,
};

#[path = "read_database_contract/lost_commit.rs"]
mod lost_commit;

/// A file database in a new cache directory of its own.
struct File {
    _root: tempfile::TempDir,
    layout: ProvenanceLayout,
    database: FileReadDatabase,
}

impl Default for File {
    fn default() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(path);
        std::fs::create_dir_all(layout.cache_dir()).unwrap();
        Self {
            database: FileReadDatabase::new(layout.clone()),
            layout,
            _root: root,
        }
    }
}

impl Harness for File {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        Box::new(FileReadDatabase::new(self.layout.clone()))
    }

    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        Box::pin(dangling_deferred_key(transaction))
    }
}

/// Leaves a row whose deferred foreign key has no parent, so `SQLite`
/// refuses the commit and rolls the transaction back.
async fn dangling_deferred_key(transaction: &mut Transaction) -> anyhow::Result<()> {
    assert_eq!(
        transaction.fetch("PRAGMA foreign_keys", Vec::new()).await?,
        vec![vec![SqlValue::Integer(1)]],
        "the file adapter checks foreign keys, so a dangling deferred key fails the commit"
    );
    for sql in [
        "CREATE TABLE IF NOT EXISTS commit_parent (id INTEGER PRIMARY KEY)",
        "CREATE TABLE IF NOT EXISTS commit_child (parent INTEGER \
         REFERENCES commit_parent (id) DEFERRABLE INITIALLY DEFERRED)",
        "INSERT INTO commit_child (parent) VALUES (1)",
    ] {
        transaction.execute(sql, Vec::new()).await?;
    }
    Ok(())
}

/// The file adapter behind another transaction type: the crate finds no
/// `SQLx` transaction in it. A reader that passes over it needs nothing
/// but the port.
struct PortOnly {
    file: File,
    database: Other<FileReadDatabase>,
}

impl Default for PortOnly {
    fn default() -> Self {
        let file = File::default();
        Self {
            database: Other(file.database.clone()),
            file,
        }
    }
}

impl Harness for PortOnly {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        Box::new(Other(FileReadDatabase::new(self.file.layout.clone())))
    }

    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        self.file.fail_commit(transaction)
    }
}

/// The file adapter with a fault: it ignores the `LIMIT` of a statement.
struct NoLimit {
    file: File,
    database: NoLimitDatabase,
}

impl Default for NoLimit {
    fn default() -> Self {
        let file = File::default();
        Self {
            database: NoLimitDatabase(file.database.clone()),
            file,
        }
    }
}

impl Harness for NoLimit {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        self.file.reopen()
    }

    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        self.file.fail_commit(transaction)
    }
}

#[derive(Debug)]
struct NoLimitDatabase(FileReadDatabase);

struct NoLimitTransaction(Transaction);

/// The statement with each `LIMIT n` read as no limit; the bound value of
/// the limit stays bound.
fn without_limit(sql: &str) -> String {
    let mut parts = sql.split(" LIMIT ");
    let mut text = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let end = part.find(char::is_whitespace).unwrap_or(part.len());
        let (limit, rest) = part.split_at(end);
        text = text + " LIMIT -1 + 0 * (" + limit + ")" + rest;
    }
    text
}

impl SqlTransaction for NoLimitTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        Box::pin(async move { self.0.fetch(&without_limit(sql), values).await })
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        self.0.execute(sql, values)
    }

    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>> {
        self.0.attempt(statements)
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        self.0.commit()
    }

    fn bound_work(&mut self, steps: u64) -> SqlFuture<'_, ()> {
        self.0.bound_work(steps)
    }

    fn max_list_values(&self) -> usize {
        self.0.max_list_values()
    }
}

impl ReadDatabase for NoLimitDatabase {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0.read(Box::new(move |lent| {
            work(Box::new(NoLimitTransaction(lent)))
        }))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0.write(work)
    }

    fn max_bound_values(&self) -> usize {
        self.0.max_bound_values()
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        self.0.cursor_key()
    }

    fn location(&self) -> String {
        self.0.location()
    }
}

macro_rules! port_cases {
    ($($module:ident :: $case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                contract::$module::$case(File::default()).await;
                contract::$module::$case(PortOnly::default()).await;
            }
        )*
    };
}

port_cases!(
    handles::the_list_handles_read_the_projection,
    handles::the_document_handle_reads_the_projection,
    handles::the_discussion_handle_reads_the_projection,
    handles::the_message_handle_reads_the_projection,
    handles::a_stopped_page_statement_is_the_page_budget_refusal,
    limits::a_bounded_read_runs_the_statements_within_its_bound,
    limits::a_limit_bounds_the_rows,
    limits::a_list_of_the_most_values_runs,
    projection::a_catch_up_makes_and_follows_the_projection,
    projection::a_schema_that_is_not_current_is_made_again,
    sequence::a_write_then_read_reads_its_write,
    sequence::a_failed_write_stops_the_read,
    sequence::a_failed_read_keeps_the_write,
    sequence::a_write_whose_commit_fails_keeps_nothing,
);

/// The limit case finds an adapter that ignores `LIMIT`.
#[tokio::test]
#[should_panic(expected = "assertion `left == right` failed")]
async fn the_limit_case_refuses_an_adapter_that_ignores_limit() {
    contract::limits::a_limit_bounds_the_rows(NoLimit::default()).await;
}

/// The file adapter with a harness whose `fail_commit` gives an error.
struct HookFails(File);

impl Harness for HookFails {
    fn database(&self) -> &dyn ReadDatabase {
        &self.0.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        self.0.reopen()
    }

    fn fail_commit<'a>(&'a self, _transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        Box::pin(async { Err::<(), _>(anyhow::anyhow!("the harness has no way to fail a commit")) })
    }
}

/// The commit case finds a harness that cannot make a commit fail and
/// gives an error in place of the failed commit.
#[tokio::test]
#[should_panic(
    expected = "the harness makes the commit fail without an error: the harness has no way to fail a commit"
)]
async fn the_commit_case_refuses_a_harness_whose_hook_fails() {
    contract::sequence::a_write_whose_commit_fails_keeps_nothing(HookFails(File::default())).await;
}

#[tokio::test]
async fn a_read_of_an_absent_database_refuses_before_its_work() {
    let file = File::default();
    contract::a_read_of_an_absent_database_refuses_before_its_work(&file).await;
    let database = file.layout.cache_db_path();
    let made: Vec<String> = ["", "-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| format!("{database}{suffix}"))
        .filter(|path| std::path::Path::new(path).exists())
        .collect();
    assert_eq!(made, Vec::<String>::new());
}

#[tokio::test]
async fn a_missing_table_is_reported_as_missing() {
    contract::a_missing_table_is_reported_as_missing(File::default()).await;
}

/// The engine reads a database whose revision table is absent as one that
/// holds no revision, through the port.
#[tokio::test]
async fn a_database_with_no_revision_table_holds_no_revision() {
    let file = File::default();
    file.database
        .write(Box::new(|mut transaction| {
            Box::pin(async move {
                transaction
                    .execute("CREATE TABLE unrelated (n INTEGER)", Vec::new())
                    .await?;
                transaction.commit().await
            })
        }))
        .await
        .unwrap();
    let mut revision = None;
    let slot = &mut revision;
    file.database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                *slot = Some(stored_revision(transaction.as_mut()).await?);
                Ok(())
            })
        }))
        .await
        .unwrap();
    assert_eq!(revision, Some(None));
}

#[tokio::test]
async fn a_write_keeps_each_value_class() {
    contract::a_write_keeps_each_value_class(File::default()).await;
}

#[tokio::test]
async fn a_failed_write_keeps_nothing() {
    contract::a_failed_write_keeps_nothing(File::default()).await;
}

#[tokio::test]
async fn a_write_without_its_commit_keeps_nothing() {
    contract::a_write_without_its_commit_keeps_nothing(File::default()).await;
}

#[tokio::test]
async fn a_write_that_fails_after_its_commit_keeps_the_commit() {
    contract::a_write_that_fails_after_its_commit_keeps_the_commit(File::default()).await;
}

#[tokio::test]
async fn a_read_keeps_no_writes() {
    contract::a_read_keeps_no_writes(File::default()).await;
}

#[tokio::test]
async fn a_read_answers_from_one_committed_state() {
    contract::sequence::a_read_answers_from_one_committed_state(File::default()).await;
}

#[tokio::test]
async fn an_attempt_keeps_all_or_none() {
    contract::an_attempt_keeps_all_or_none(File::default()).await;
}

#[tokio::test]
async fn the_projection_statement_forms_run() {
    contract::the_projection_statement_forms_run(File::default()).await;
}

#[test]
fn the_cursor_key_stays_the_same() {
    contract::the_cursor_key_stays_the_same(&File::default());
}

/// Makes the database of `file`, with one table.
async fn made(file: &File) {
    file.database
        .write(Box::new(|mut transaction| {
            Box::pin(async move {
                transaction
                    .execute("CREATE TABLE made (n INTEGER)", Vec::new())
                    .await?;
                transaction.commit().await
            })
        }))
        .await
        .unwrap();
}

/// A query whose work grows with `n`: it counts from 1 to `n`.
const COUNT_TO: &str = "WITH RECURSIVE count_to (n) AS (SELECT 1 UNION ALL \
                        SELECT n + 1 FROM count_to WHERE n < ?) SELECT COUNT(*) FROM count_to";

/// The count to `n` in one read of the file, after the work bound `bound`
/// when there is one.
async fn count_to(file: &File, bound: Option<u64>, n: i64) -> anyhow::Result<Vec<SqlRow>> {
    let mut rows = None;
    let slot = &mut rows;
    file.database
        .read(Box::new(move |mut transaction| {
            Box::pin(async move {
                if let Some(steps) = bound {
                    transaction.bound_work(steps).await?;
                }
                *slot = Some(
                    transaction
                        .fetch(COUNT_TO, vec![SqlValue::Integer(n)])
                        .await,
                );
                Ok(())
            })
        }))
        .await?;
    rows.expect("the read ran its work")
}

/// The file stops a statement while it runs once its work passes the
/// bound, and the error is `OverBudget`. Without the bound, the same
/// statement runs. The shared contract does not hold this, because it
/// does not yet say what another adapter must do at the bound.
#[tokio::test]
async fn the_file_stops_a_statement_past_the_bound() {
    let file = File::default();
    made(&file).await;
    let stopped = count_to(&file, Some(100_000), 1_000_000).await.unwrap_err();
    assert!(
        stopped.downcast_ref::<OverBudget>().is_some(),
        "not OverBudget: {stopped:#}"
    );
    let unbounded = count_to(&file, None, 1_000_000).await.unwrap();
    assert_eq!(unbounded, vec![vec![SqlValue::Integer(1_000_000)]]);
}

/// The file puts at most 500 values in one list, as the readers did before
/// they ran on the port, so a chunked list runs the same statements.
#[tokio::test]
async fn the_file_puts_500_values_in_one_list() {
    let file = File::default();
    made(&file).await;
    let mut most = 0;
    let slot = &mut most;
    file.database
        .read(Box::new(move |transaction| {
            Box::pin(async move {
                *slot = transaction.max_list_values();
                Ok(())
            })
        }))
        .await
        .unwrap();
    assert_eq!(most, 500);
}
