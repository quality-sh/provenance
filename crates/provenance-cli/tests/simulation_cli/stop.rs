//! The stop of a CLI write at a point of its publish that the test sees from
//! outside the write process.
//!
//! A write lands as one whole set. Under the publication lock, the process
//! makes its transaction directory `<pid>-<time>` below
//! `.provenance/cache/import-transactions`, copies the state into it and
//! applies the write there. Then it writes the publication marker
//! `.provenance/cache/import-publication.json`, moves the live state into
//! the transaction, installs the new state, removes the transaction, and
//! removes the marker. After that, it releases the lock and prints its
//! answer. The next entry to the section completes or rolls back a publish
//! that a stop interrupted.
//!
//! The test lists the two directories and kills the process when it sees
//! the point. A listing opens no file of the write. A kill does not let the
//! process remove its transaction or its marker, so after the kill they tell
//! if the stop landed inside the publish.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::Instant;

/// The name of the publication marker in `.provenance/cache`.
const MARKER: &str = "import-publication.json";

/// A point of the publish where a stop kills the write process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    /// The transaction of the write appears: the process copies the state
    /// into it, and the live state does not move.
    Transaction,
    /// The publication marker appears in the transaction of the write: the
    /// process moves the live state into the transaction next.
    Marker,
}

impl Point {
    /// The point of a stop at `at`: an even `at` gives the transaction, an
    /// odd `at` the marker.
    pub const fn of(at: u8) -> Self {
        if at.is_multiple_of(2) {
            Self::Transaction
        } else {
            Self::Marker
        }
    }
}

/// Where a stop landed, read after the kill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    /// The transaction or the marker of the write stayed: the stop landed
    /// inside the publish.
    Publish,
    /// The process had removed its transaction and its marker and had not
    /// printed its answer: the stop landed after the publish.
    AfterPublish,
}

/// The transactions and the marker of one repository, as the test sees them
/// while one write process runs.
#[derive(Clone)]
pub struct Watch {
    cache: PathBuf,
    /// The transactions before the process started.
    before: BTreeSet<OsString>,
    /// The start of the name of a transaction of the process.
    prefix: String,
}

impl Watch {
    /// Notes the transactions of the repository at `root` before a write
    /// process starts, so that a transaction that an earlier stop left does
    /// not count.
    pub fn before(root: &Path) -> Self {
        let cache = root.join(".provenance").join("cache");
        let before = names(&cache.join("import-transactions"));
        Self {
            cache,
            before,
            prefix: String::new(),
        }
    }

    /// Watches the transactions of the process `pid`.
    pub fn of(&self, pid: u32) -> Self {
        Self {
            prefix: format!("{pid}-"),
            ..self.clone()
        }
    }

    /// Tells if a transaction of the process is in the repository.
    fn transaction(&self) -> bool {
        let names = names(&self.cache.join("import-transactions"));
        names.iter().any(|name| {
            !self.before.contains(name) && name.to_string_lossy().starts_with(&self.prefix)
        })
    }

    /// Tells if the marker is in the repository.
    fn marker(&self) -> bool {
        names(&self.cache).contains(&OsString::from(MARKER))
    }

    /// Waits until the process `child` reaches `point`, and tells if it did.
    /// It does not wait for a process that ends first, or for more than
    /// `limit`. A marker counts only after the transaction of the process
    /// appeared: the process removes a marker that an earlier stop left
    /// before it makes its transaction.
    pub fn wait(&self, child: &mut Child, point: Point, limit: Instant) -> bool {
        let mut transaction = false;
        while Instant::now() < limit && matches!(child.try_wait(), Ok(None)) {
            transaction = transaction || self.transaction();
            let reached = match point {
                Point::Transaction => transaction,
                Point::Marker => transaction && self.marker(),
            };
            if reached {
                return true;
            }
            std::thread::yield_now();
        }
        false
    }

    /// Where the stop of a process that reached its point landed. Call it
    /// after the process ended.
    pub fn landing(&self) -> Landing {
        if self.transaction() || self.marker() {
            Landing::Publish
        } else {
            Landing::AfterPublish
        }
    }
}

/// The names in `directory`, or none when it does not exist.
fn names(directory: &Path) -> BTreeSet<OsString> {
    std::fs::read_dir(directory)
        .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default()
}
