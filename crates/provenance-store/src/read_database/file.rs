//! The file adapter: the CLI's `SQLite` file under `.provenance/cache`.

use super::{ReadDatabase, ReadWork, SqlFuture, WriteWork};
use crate::layout::ProvenanceLayout;

/// The read database in the cache directory of one layout.
#[derive(Debug, Clone)]
pub struct FileReadDatabase {
    layout: ProvenanceLayout,
}

impl FileReadDatabase {
    pub const fn new(layout: ProvenanceLayout) -> Self {
        Self { layout }
    }
}

impl ReadDatabase for FileReadDatabase {
    fn read<'a>(&'a self, _work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async { Err(crate::refusal::fault!("no file adapter yet")) })
    }

    fn write<'a>(&'a self, _work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async { Err(crate::refusal::fault!("no file adapter yet")) })
    }

    fn max_bound_values(&self) -> usize {
        999
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        Err(crate::refusal::fault!("no file adapter yet"))
    }

    fn location(&self) -> String {
        self.layout.cache_db_path().to_string()
    }
}
