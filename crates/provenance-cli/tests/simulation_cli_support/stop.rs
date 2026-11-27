//! The stop of a CLI write at a point of its publish that the test sees from
//! outside the write process.
//!
//! A write of two keys or more publishes through a transaction directory and
//! a publication marker. The order of these steps and the phases of the
//! marker are in `provenance_store::storage::local::whole_set::land`. A write
//! of one key writes a temporary file in the cache, syncs it, and renames it
//! onto the key (`durability::write_synced_state_file`); the store tells its
//! name with `is_state_temporary_name`. The test lists the directory of the
//! transactions, the directory of the marker and the cache, and kills the
//! write process when it sees the point of the stop. A one-key write has one
//! point: its temporary file appears in the cache. The rename comes one file
//! sync later, so the kill can land before or after it. That file can live
//! for less time than the gap between two looks, so the test also gets the
//! file events of the cache: they stay in their queue after the rename, so
//! the test sees the point even then. A kill does not let the process remove
//! its transaction, its marker or its temporary file. So after the kill,
//! these entries, the phase of the marker and the live state tell where the
//! stop landed.

use super::{answer, done, effect, layer, panicked, parse, refusal_kind, request, Running};
use crate::fixture::state_entries;
use provenance_simulation::generate::Task;
use provenance_simulation::{stats, Done};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::storage::local::is_state_temporary_name;
use provenance_store::storage::StateKey;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[path = "stop_events.rs"]
mod events;
use events::CacheEvents;

/// The time between two looks at the disk. The wait sleeps between its
/// looks, so that it does not keep a core busy.
const LOOK_INTERVAL: Duration = Duration::from_micros(100);

/// The state of a repository, or the error of its read.
pub type Entries = Result<BTreeMap<StateKey, Vec<u8>>, String>;

/// Each file below a directory, with its bytes. A read of the state as
/// [`Entries`] does not show a file that is not a state key, so the probe
/// also compares the files.
pub type Files = BTreeMap<PathBuf, Vec<u8>>;

/// A point of the publish where a stop kills the write process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    /// The transaction of the write appears, or the temporary file of a
    /// one-key write appears in the cache.
    First,
    /// The publication marker appears while the transaction of the write
    /// exists. A one-key write has no second point: the stop kills it at
    /// its first point.
    Second,
}

impl Point {
    /// The point of a stop at `at`: an even `at` gives the first point, an
    /// odd `at` the second point.
    pub const fn of(at: u8) -> Self {
        if at.is_multiple_of(2) {
            Self::First
        } else {
            Self::Second
        }
    }
}

/// The entry that the write made and that the test saw.
pub enum Made {
    /// The transaction of a write of two keys or more.
    Transaction(PathBuf),
    /// The temporary file of a one-key write, in the cache.
    Temporary(PathBuf),
}

/// Where a stop landed in the publish of its write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    /// The transaction of the write stays and no marker is there, so the
    /// live state did not move.
    BeforeMarker,
    /// The marker names the transaction of the write, and the new state is
    /// not installed.
    BeforeInstall,
    /// The new state is installed: the marker says so, or the write removed
    /// its transaction and its marker.
    AfterInstall,
    /// The temporary file of the key is in the cache; the key did not
    /// change.
    BeforeRename,
    /// The key has its new bytes; the write did not answer.
    AfterRename,
}

/// A stop that killed its write before the write gave its answer.
pub struct Landed {
    pub landing: Landing,
    /// The transaction of the write, or the temporary file of a one-key
    /// write.
    pub made: PathBuf,
    /// The state before the write started.
    pub before: Entries,
    /// The files below the state directory and their bytes, before the
    /// write started and after the kill.
    pub files: (Files, Files),
    /// A live state is there after the kill.
    pub live_state: bool,
}

/// What a stop that a case drew did.
pub enum Stop {
    /// No write of the engine waited, so the stop had no write to kill.
    NoWrite,
    /// The simulation sent no process for the write: the CLI layer has no
    /// route for it, or the read of its etag refused.
    NotSent,
    /// The stop killed the write before its answer.
    Landed(Landed),
    /// The write gave its answer before the kill. `before` is the state
    /// before the write started.
    Answered { before: Entries },
    /// The write ended before the test saw a point.
    Ended,
    /// The test saw the point of the write only after the write ended, and
    /// the write exited with no answer.
    Exited,
    /// The write did not end in the process limit, so its case breaks L0.
    Limit,
}

const BEFORE_MARKER: &str = "stops before the marker";
const BEFORE_INSTALL: &str = "stops after the marker and before the install";
const AFTER_INSTALL: &str = "stops after the install and before the answer";
const BEFORE_RENAME: &str = "stops before the rename of the key";
const AFTER_RENAME: &str = "stops after the rename of the key and before the answer";
const NO_LIVE_STATE: &str = "stops that left no live state";
const ANSWERED: &str = "stops after the answer";
const ENDED: &str = "stops of a write that ended before the test saw a point";
const EXITED: &str = "stops of a write that exited with no answer";
const UNSENT: &str = "stops of a write that the simulation did not send";
const NO_WRITE: &str = "stops with no waiting write of the engine";
const LIMIT: &str = "stops of a write that did not end in its limit";

/// Each row of the job summary that counts stops.
const ROWS: [&str; 12] = [
    BEFORE_MARKER,
    BEFORE_INSTALL,
    AFTER_INSTALL,
    BEFORE_RENAME,
    AFTER_RENAME,
    NO_LIVE_STATE,
    ANSWERED,
    ENDED,
    EXITED,
    UNSENT,
    NO_WRITE,
    LIMIT,
];

impl Stop {
    /// The row of the job summary that counts this stop. It also tells what
    /// the stop did.
    pub const fn row(&self) -> &'static str {
        match self {
            Self::NoWrite => NO_WRITE,
            Self::NotSent => UNSENT,
            Self::Landed(landed) => match landed.landing {
                Landing::BeforeMarker => BEFORE_MARKER,
                Landing::BeforeInstall => BEFORE_INSTALL,
                Landing::AfterInstall => AFTER_INSTALL,
                Landing::BeforeRename => BEFORE_RENAME,
                Landing::AfterRename => AFTER_RENAME,
            },
            Self::Answered { .. } => ANSWERED,
            Self::Ended => ENDED,
            Self::Exited => EXITED,
            Self::Limit => LIMIT,
        }
    }

    /// Counts the stop in the job summary.
    pub fn count(&self) {
        stats::add(&layer(), self.row(), 1);
        if let Self::Landed(landed) = self {
            stats::add(&layer(), NO_LIVE_STATE, u64::from(!landed.live_state));
        }
    }
}

/// Adds 0 to each stop row, so that the rows show on a night with no stop.
pub fn count_none() {
    for row in ROWS {
        stats::add(&layer(), row, 0);
    }
}

/// Runs the write of `task` alone on the repository at `root`, with the
/// layout `layout`, and kills it when it reaches the point of a stop at
/// `at`. Gives what the write did, when the simulation sent it, and what the
/// stop did.
pub fn run_stopped(
    root: &Path,
    layout: &ProvenanceLayout,
    task: &Task,
    at: u8,
) -> (Option<Done>, Stop) {
    let request = match request(root, task) {
        None => return (None, Stop::NotSent),
        Some(Err(refusal)) => return (Some(done(task, Err(refusal), false)), Stop::NotSent),
        Some(Ok(request)) => request,
    };
    let what = format!("write {}", request.path);
    let before = state_entries(root);
    let files_before = files(layout.state_dir().as_std_path());
    let watch = Watch::new(layout);
    let mut running = Running::start(request.spawn(root));
    let Some((made, ended)) = watch.reach(&mut running, Point::of(at)) else {
        let output = match running.wait(&what) {
            Ok(output) => output,
            Err(text) => return (Some(done(task, Err(text), false)), Stop::Limit),
        };
        let result = answer(&output).and_then(|data| effect(task, &request, &data));
        return (Some(done(task, result, false)), Stop::Ended);
    };
    let output = running.kill(&what);
    // A process gives its answer at its end: the data of a success on
    // standard output, or a refusal with its kind on standard error.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let refused = refusal_kind(stderr.trim()) != "no_kind";
    if !output.stdout.is_empty() || output.status.success() || refused || panicked(&output) {
        let result = if output.stdout.is_empty() {
            answer(&output)
        } else {
            Ok(parse(&output.stdout)["data"].clone())
        };
        let result = result.and_then(|data| effect(task, &request, &data));
        return (Some(done(task, result, false)), Stop::Answered { before });
    }
    if ended {
        let result = answer(&output).and_then(|data| effect(task, &request, &data));
        return (Some(done(task, result, false)), Stop::Exited);
    }
    let landing = match &made {
        Made::Transaction(transaction) => landing(layout, transaction),
        Made::Temporary(temporary) if temporary.exists() => Landing::BeforeRename,
        Made::Temporary(_) => Landing::AfterRename,
    };
    let landed = Landed {
        landing,
        made: match made {
            Made::Transaction(path) | Made::Temporary(path) => path,
        },
        before,
        files: (files_before, files(layout.state_dir().as_std_path())),
        live_state: layout.state_dir().exists(),
    };
    (
        Some(done(task, Err("stopped".into()), true)),
        Stop::Landed(landed),
    )
}

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
    /// that it made and if the write had ended when the test saw the point.
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
    pub fn reach(&self, running: &mut Running, point: Point) -> Option<(Made, bool)> {
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
                return made.map(|made| (made, ended));
            }
            if ended {
                return None;
            }
            std::thread::sleep(LOOK_INTERVAL);
        }
    }
}

/// Where the stop of the write with the transaction `transaction` landed,
/// read after the kill.
fn landing(layout: &ProvenanceLayout, transaction: &Path) -> Landing {
    let bytes = match std::fs::read(layout.publication_marker_path()) {
        Ok(bytes) => bytes,
        // With no marker, the transaction stays only before the marker: the
        // write removes its transaction before its marker.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return if transaction.exists() {
                Landing::BeforeMarker
            } else {
                Landing::AfterInstall
            };
        }
        Err(error) => panic!("the marker does not read: {error}"),
    };
    let marker = parse(&bytes);
    let named = marker["transaction_dir"].as_str().map(Path::new);
    assert_eq!(
        named.and_then(Path::file_name),
        transaction.file_name(),
        "the marker {marker} does not name the transaction of the stopped write"
    );
    // The write moves the live state into its transaction before the phase
    // `backup_created`, and installs the new state after it. So in that
    // phase, a live state is the new state.
    match marker["phase"].as_str() {
        Some("prepared") => Landing::BeforeInstall,
        Some("backup_created") if !layout.state_dir().exists() => Landing::BeforeInstall,
        Some("backup_created" | "published") => Landing::AfterInstall,
        _ => panic!("the marker {marker} has no phase that the test knows"),
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

/// Each file below `directory` with its bytes, or none when it does not
/// exist.
fn files(directory: &Path) -> Files {
    let mut found = Files::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            found.extend(files(&path));
        } else if let Ok(bytes) = std::fs::read(&path) {
            found.insert(path, bytes);
        }
    }
    found
}

/// The names in `directory`, or none when it does not exist.
fn names(directory: &Path) -> BTreeSet<OsString> {
    std::fs::read_dir(directory)
        .map_or_default(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
}
