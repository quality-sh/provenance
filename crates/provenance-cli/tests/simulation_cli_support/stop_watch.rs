//! The watch of the disk while a stopped write runs: the transactions, the
//! marker and the temporary files of one repository, and the file events of
//! its cache.

use super::{Made, Point, LOOK_INTERVAL};
use crate::process::Running;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::storage::local::is_state_temporary_name;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[path = "stop_events.rs"]
mod events;
use events::CacheEvents;

/// The transactions, the marker and the temporary files of one repository,
/// as the test sees them while its stopped write runs. The write runs alone,
/// so a transaction or a temporary file that was not there before the write
/// started is one of the write.
pub struct Watch<'a> {
    layout: &'a ProvenanceLayout,
    /// The transactions before the write started.
    before: BTreeSet<OsString>,
    /// The names of the temporary files of one-key writes in the cache
    /// before the write started.
    temporary_before: BTreeSet<OsString>,
    /// The file events of the cache.
    events: CacheEvents,
}

impl<'a> Watch<'a> {
    /// Starts the watch of the cache. An earlier stop before a rename leaves
    /// its temporary file until the next entry to the publication section,
    /// so a temporary file that is there now is not one of the write.
    pub fn new(layout: &'a ProvenanceLayout) -> Self {
        let before = names(layout.import_transactions_dir().as_std_path());
        let temporary_before: BTreeSet<_> = state_temporaries(layout)
            .iter()
            .filter_map(|path| path.file_name().map(OsString::from))
            .collect();
        let cache = layout.cache_dir();
        Self {
            layout,
            before,
            events: CacheEvents::start(cache.as_std_path(), temporary_before.clone()),
            temporary_before,
        }
    }

    /// A temporary file of a one-key write that was not there before the
    /// write started: one that a look at the cache sees, or else one that an
    /// event named. The events of some systems come some time after the
    /// change, so the look finds the file while the write runs more often.
    fn new_temporary(&self) -> Option<PathBuf> {
        state_temporaries(self.layout)
            .into_iter()
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| !self.temporary_before.contains(name))
            })
            .or_else(|| self.events.new_temporary())
    }

    /// The name of a transaction that was not there before the write started.
    fn new_transaction(&self) -> Option<OsString> {
        let names = names(self.layout.import_transactions_dir().as_std_path());
        names.into_iter().find(|name| !self.before.contains(name))
    }

    /// Tells if the marker is there. A listing of its directory opens no
    /// file of the write.
    fn marker(&self) -> bool {
        let marker = self.layout.publication_marker_path();
        let (Some(directory), Some(name)) = (marker.parent(), marker.file_name()) else {
            return false;
        };
        names(directory.as_std_path()).contains(&OsString::from(name))
    }

    /// Waits until the write `running` reaches `point`, and gives the entry
    /// that it made.
    /// When the write ends, the test takes each event up to that time and
    /// looks one more time; it gives `None` when that look does not find the
    /// point. When the deadline is past and the write still runs, it gives
    /// `None`, so that the wait reports the limit. A marker counts only
    /// after the transaction of the write appeared: each entry to the
    /// publication section recovers a marker that an earlier stop left,
    /// before the write makes its transaction
    /// (`LocalSectionLock::acquire_checked`). A write that made a
    /// transaction makes no temporary file of a one-key write. A one-key
    /// write is reached when the test sees its temporary file, at each
    /// point.
    pub fn reach(&self, running: &mut Running, point: Point) -> Option<Made> {
        let mut made = None;
        loop {
            let runs = running.runs();
            let ended = !runs && running.ended();
            if !runs && !ended {
                return None;
            }
            if ended {
                self.events.settle();
            }
            made = made.or_else(|| {
                self.new_transaction()
                    .map(|name| {
                        let directory = self.layout.import_transactions_dir();
                        Made::Transaction(directory.as_std_path().join(name))
                    })
                    .or_else(|| self.new_temporary().map(Made::Temporary))
            });
            let reached = match &made {
                None => false,
                Some(Made::Transaction(_)) if point == Point::Second => self.marker(),
                Some(_) => true,
            };
            if reached {
                return made;
            }
            if ended {
                return None;
            }
            std::thread::sleep(LOOK_INTERVAL);
        }
    }
}

/// The paths of the temporary files of one-key writes in the cache of
/// `layout`: each file directly in the cache whose name the store gives to
/// such a file.
fn state_temporaries(layout: &ProvenanceLayout) -> BTreeSet<PathBuf> {
    let cache = layout.cache_dir();
    let Ok(entries) = std::fs::read_dir(&cache) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(is_state_temporary_name)
                && entry.file_type().is_ok_and(|kind| kind.is_file())
        })
        .map(|entry| entry.path())
        .collect()
}

/// The names in `directory`, or none when it does not exist.
fn names(directory: &Path) -> BTreeSet<OsString> {
    std::fs::read_dir(directory)
        .map_or_default(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
}
