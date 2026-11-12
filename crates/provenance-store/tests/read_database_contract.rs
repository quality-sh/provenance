//! The read database contract on the file adapter.
//!
//! No Rule: the plan proposes `rule_read_database_adapters_meet_contract`
//! for the owner's review. Until it is approved, these tests hold the
//! contract while the projection moves behind the port.

use camino::Utf8PathBuf;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::stamp::stored_revision;
use provenance_store::read_database::contract::{self, handles, Harness};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};

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
}

/// The file adapter with only the port in view: its transactions give
/// no `SQLx` transaction, as the adapter of a Durable Object has none. A
/// reader that passes over it needs nothing but the port.
struct PortOnly {
    file: File,
    database: PortOnlyDatabase,
}

impl Default for PortOnly {
    fn default() -> Self {
        let file = File::default();
        Self {
            database: PortOnlyDatabase(file.database.clone()),
            file,
        }
    }
}

#[derive(Debug)]
struct PortOnlyDatabase(FileReadDatabase);

struct PortOnlyTransaction(Transaction);

impl Harness for PortOnly {
    fn database(&self) -> &dyn ReadDatabase {
        &self.database
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        self.file.reopen()
    }
}

fn hidden(transaction: Transaction) -> Transaction {
    Box::new(PortOnlyTransaction(transaction))
}

impl ReadDatabase for PortOnlyDatabase {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0
            .read(Box::new(move |transaction| work(hidden(transaction))))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0
            .write(Box::new(move |transaction| work(hidden(transaction))))
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

impl SqlTransaction for PortOnlyTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, Vec<SqlRow>> {
        self.0.fetch(sql, values)
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, u64> {
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
}

macro_rules! handle_cases {
    ($($case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                handles::$case(File::default()).await;
                handles::$case(PortOnly::default()).await;
            }
        )*
    };
}

handle_cases!(
    the_list_handles_read_the_projection,
    the_document_handle_reads_the_projection,
    the_discussion_handle_reads_the_projection,
    the_message_handle_reads_the_projection,
);

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
                    .execute("CREATE TABLE unrelated (n INTEGER)", &[])
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
    contract::a_read_answers_from_one_committed_state(File::default()).await;
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
