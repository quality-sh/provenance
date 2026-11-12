//! The read database contract on the file adapter.
//!
//! No Rule: the plan proposes `rule_read_database_adapters_meet_contract`
//! for the owner's review. Until it is approved, these tests hold the
//! contract while the projection moves behind the port.

use camino::Utf8PathBuf;
use provenance_store::layout::ProvenanceLayout;
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
