//! The read database contract on the file adapter.
//!
//! No Rule: the plan proposes `rule_read_database_adapters_meet_contract`
//! for the owner's review. Until it is approved, these tests hold the
//! contract while the projection moves behind the port.

use camino::Utf8PathBuf;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::stamp::stored_revision;
use provenance_store::read_database::contract::{self, Harness};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::read_database::ReadDatabase;

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
