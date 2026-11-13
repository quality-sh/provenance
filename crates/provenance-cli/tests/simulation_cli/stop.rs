//! The stop of a CLI write at a point of its publish that the test sees from
//! outside the write process.
//!
//! A write publishes through a transaction directory and a publication
//! marker. The order of these steps and the phases of the marker are in
//! `provenance_store::storage::local::whole_set::land`. The test lists the
//! directory of the transactions and the directory of the marker, and kills
//! the write process when it sees the point of the stop. A kill does not let
//! the process remove its transaction or its marker. So after the kill, the
//! transaction, the marker, its phase and the live state tell where the stop
//! landed.

use super::{answer, done, effect, layer, panicked, parse, refusal_kind, request, Running};
use crate::fixture::state_entries;
use provenance_simulation::generate::Task;
use provenance_simulation::{stats, Done};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::storage::StateKey;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The time between two looks at the disk. The wait sleeps between its
/// looks, so that it does not keep a core busy.
const LOOK_INTERVAL: Duration = Duration::from_micros(100);

/// The state of a repository, or the error of its read.
pub type Entries = Result<BTreeMap<StateKey, Vec<u8>>, String>;

/// A point of the publish where a stop kills the write process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    /// The transaction of the write appears.
    Transaction,
    /// The publication marker appears while the transaction of the write
    /// exists.
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
}

/// A stop that killed its write before the write gave its answer.
pub struct Landed {
    pub landing: Landing,
    /// The transaction of the write.
    pub transaction: PathBuf,
    /// The state before the write started.
    pub before: Entries,
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
    /// The write gave its answer before the kill.
    Answered,
    /// The write ended before its stop point and changed no state.
    EndedBeforePoint,
    /// The write changed the state, and the test did not see its stop point.
    PointNotSeen,
    /// The write did not end in the process limit, so its case breaks L0.
    Limit,
}

const BEFORE_MARKER: &str = "stops before the marker";
const BEFORE_INSTALL: &str = "stops after the marker and before the install";
const AFTER_INSTALL: &str = "stops after the install and before the answer";
const NO_LIVE_STATE: &str = "stops that left no live state";
const ANSWERED: &str = "stops after the answer";
const ENDED: &str = "stops of a write that ended before its stop point";
const NOT_SEEN: &str = "stops whose point the test did not see";
const UNSENT: &str = "stops of a write that the simulation did not send";
const NO_WRITE: &str = "stops with no waiting write of the engine";
const LIMIT: &str = "stops of a write that did not end in its limit";

/// Each row of the job summary that counts stops.
const ROWS: [&str; 10] = [
    BEFORE_MARKER,
    BEFORE_INSTALL,
    AFTER_INSTALL,
    NO_LIVE_STATE,
    ANSWERED,
    ENDED,
    NOT_SEEN,
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
            },
            Self::Answered => ANSWERED,
            Self::EndedBeforePoint => ENDED,
            Self::PointNotSeen => NOT_SEEN,
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
    let watch = Watch::new(layout);
    let mut running = Running::start(request.spawn(root));
    let Some(transaction) = watch.reach(&mut running, Point::of(at)) else {
        let output = match running.wait(&what) {
            Ok(output) => output,
            Err(text) => return (Some(done(task, Err(text), false)), Stop::Limit),
        };
        let result = answer(&output).and_then(|data| effect(task, &request, &data));
        let stop = if state_entries(root) == before {
            Stop::EndedBeforePoint
        } else {
            Stop::PointNotSeen
        };
        return (Some(done(task, result, false)), stop);
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
        return (Some(done(task, result, false)), Stop::Answered);
    }
    let landed = Landed {
        landing: landing(layout, &transaction),
        transaction,
        before,
        live_state: layout.state_dir().exists(),
    };
    (
        Some(done(task, Err("stopped".into()), true)),
        Stop::Landed(landed),
    )
}

/// The transactions and the marker of one repository, as the test sees them
/// while its stopped write runs. The write runs alone, so a transaction that
/// was not there before the write started is a transaction of the write.
struct Watch<'a> {
    layout: &'a ProvenanceLayout,
    /// The transactions before the write started.
    before: BTreeSet<OsString>,
}

impl<'a> Watch<'a> {
    fn new(layout: &'a ProvenanceLayout) -> Self {
        let before = names(layout.import_transactions_dir().as_std_path());
        Self { layout, before }
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

    /// Waits until the write `running` reaches `point`, and gives the path
    /// of its transaction. Gives `None` when the write ends first or its
    /// deadline is past. A marker counts only after the transaction of the
    /// write appeared: each entry to the publication section recovers a
    /// marker that an earlier stop left, before the write makes its
    /// transaction (`LocalSectionLock::acquire_checked`).
    fn reach(&self, running: &mut Running, point: Point) -> Option<PathBuf> {
        let mut transaction = None;
        while running.runs() {
            transaction = transaction.or_else(|| self.new_transaction());
            if let Some(name) = &transaction {
                if point == Point::Transaction || self.marker() {
                    return Some(
                        self.layout
                            .import_transactions_dir()
                            .as_std_path()
                            .join(name),
                    );
                }
            }
            std::thread::sleep(LOOK_INTERVAL);
        }
        None
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

/// The names in `directory`, or none when it does not exist.
fn names(directory: &Path) -> BTreeSet<OsString> {
    std::fs::read_dir(directory)
        .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
        .unwrap_or_default()
}
