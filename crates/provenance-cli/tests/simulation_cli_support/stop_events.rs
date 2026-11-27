//! The file events of the cache while a stopped write runs.
//!
//! The temporary file of a one-key write can live for less time than the
//! gap between two looks at the disk. The system keeps its file events in a
//! queue after the rename, so these events tell that the write made its
//! temporary file even when no look saw it. A file that the test writes in
//! the cache is a barrier: when its event comes, each event of a change
//! before it came too.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use provenance_store::storage::local::is_state_temporary_name;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// The barrier before the write starts, so that the watch sends events.
const START_BARRIER: &str = "simulation-watch-start";

/// The barrier after the write ends, so that each event of the write came.
const END_BARRIER: &str = "simulation-watch-end";

/// The limit of the wait for the event of a barrier.
const BARRIER_LIMIT: Duration = Duration::from_secs(60);

/// The watch of the cache of one repository for one write.
pub struct CacheEvents {
    cache: PathBuf,
    /// The names of the temporary files of one-key writes in the cache
    /// before the write started.
    temporary_before: BTreeSet<OsString>,
    /// The file events of the cache, in the order of the changes.
    events: Receiver<notify::Result<Event>>,
    /// It sends the events while it lives.
    _watcher: RecommendedWatcher,
    /// The first new temporary file that an event named.
    temporary: RefCell<Option<PathBuf>>,
}

impl CacheEvents {
    /// Starts the watch of `cache`, and waits until its events come.
    /// `temporary_before` names the temporary files that are not of the
    /// write.
    pub fn start(cache: &Path, temporary_before: BTreeSet<OsString>) -> Self {
        std::fs::create_dir_all(cache).expect("the test makes the cache");
        let (sender, events) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .expect("a watcher of the file events");
        watcher
            .watch(cache, RecursiveMode::NonRecursive)
            .expect("a watch of the cache");
        let watch = Self {
            cache: cache.to_path_buf(),
            temporary_before,
            events,
            _watcher: watcher,
            temporary: RefCell::new(None),
        };
        watch.wait_for(START_BARRIER);
        watch
    }

    /// A temporary file of a one-key write that was not there before the
    /// write started, from the events that came.
    pub fn new_temporary(&self) -> Option<PathBuf> {
        while let Ok(event) = self.events.try_recv() {
            self.note(event, END_BARRIER);
        }
        self.temporary.borrow().clone()
    }

    /// Takes each event of a change before this call. Call it after the end
    /// of the write.
    pub fn settle(&self) {
        self.wait_for(END_BARRIER);
    }

    /// Writes the file `barrier` in the cache, takes each event up to the
    /// event of that file, and removes the file.
    fn wait_for(&self, barrier: &str) {
        let path = self.cache.join(barrier);
        std::fs::write(&path, b"").expect("the test writes its barrier file");
        let deadline = Instant::now() + BARRIER_LIMIT;
        loop {
            let event = self
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("the file event of the barrier comes");
            if self.note(event, barrier) {
                break;
            }
        }
        std::fs::remove_file(&path).expect("the test removes its barrier file");
    }

    /// Notes the first new temporary file that `event` names, and tells if
    /// it names `barrier`. Each kind of event counts: the name of a
    /// temporary file is new for each write, so only the write made it.
    fn note(&self, event: notify::Result<Event>, barrier: &str) -> bool {
        let event = event.expect("a file event of the cache");
        assert!(
            !event.need_rescan(),
            "the system dropped file events of the cache"
        );
        let mut found = false;
        for name in event.paths.iter().filter_map(|path| path.file_name()) {
            found |= name == barrier;
            let new = name.to_str().is_some_and(is_state_temporary_name)
                && !self.temporary_before.contains(name);
            let mut temporary = self.temporary.borrow_mut();
            if new && temporary.is_none() {
                *temporary = Some(self.cache.join(name));
            }
        }
        found
    }
}
