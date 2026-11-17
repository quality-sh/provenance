use notify::Event;
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::{
    sync::{mpsc, watch, Notify},
    time::Instant,
};

/// The time with no file event after which the state files count as settled.
const QUIET: Duration = Duration::from_millis(500);
/// The longest wait for the state files to settle before a read that follows a fault.
const SETTLE_LIMIT: Duration = Duration::from_secs(5);

/// The file events of the review state, merged into notices that tell the observer what to read.
pub(super) struct FileEvents {
    receiver: mpsc::Receiver<notify::Result<Event>>,
    lost: Arc<AtomicBool>,
    wake: Arc<Notify>,
    layout: ProvenanceLayout,
}

/// What the file events since the last read ask of the next read.
#[derive(Default)]
pub(super) struct Notice {
    /// A watched directory changed, so the watches must be made again.
    pub reattach: bool,
    /// Events were lost, so the next read cannot be compared with the last one.
    pub lost: bool,
}

impl Notice {
    fn merge(&mut self, other: Self) {
        self.reattach |= other.reattach;
        self.lost |= other.lost;
    }
}

impl FileEvents {
    /// Makes the event queue and the handler that the file watcher calls for each event.
    pub fn new(
        layout: ProvenanceLayout,
    ) -> (impl FnMut(notify::Result<Event>) + Send + 'static, Self) {
        let (sender, receiver) = mpsc::channel(256);
        let lost = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Notify::new());
        let handler_layout = layout.clone();
        let handler_lost = lost.clone();
        let handler_wake = wake.clone();
        let handler = move |event: notify::Result<Event>| {
            if let Ok(event) = &event {
                if !event.need_rescan()
                    && (event.kind.is_access() || !relevant(event, &handler_layout))
                {
                    return;
                }
            }
            if sender.try_send(event).is_err() {
                handler_lost.store(true, Ordering::Release);
                handler_wake.notify_one();
            }
        };
        let events = Self {
            receiver,
            lost,
            wake,
            layout,
        };
        (handler, events)
    }

    /// Waits for the next file event and merges it with the events already queued.
    pub async fn next(&mut self, stopped: &mut watch::Receiver<bool>) -> Option<Notice> {
        if *stopped.borrow() {
            return None;
        }
        let first = tokio::select! {
            event = self.receiver.recv() => event?,
            () = self.wake.notified() => {
                return Some(Notice { lost: self.lost.swap(false, Ordering::AcqRel), reattach: true });
            }
            _ = stopped.changed() => return None,
        };
        let mut notice = self.classify(first);
        while let Ok(event) = self.receiver.try_recv() {
            notice.merge(self.classify(event));
        }
        notice.lost |= self.lost.swap(false, Ordering::AcqRel);
        Some(notice)
    }

    /// Waits until no file event comes for [`QUIET`], or at most [`SETTLE_LIMIT`], so that
    /// the read after a fault does not see a write that is only partly done. Returns the
    /// merged notice of the events that the wait consumed.
    #[rule("rule_review_host_reads_settled_state")]
    pub async fn settle(&mut self, stopped: &mut watch::Receiver<bool>) -> Notice {
        let limit = Instant::now() + SETTLE_LIMIT;
        let mut notice = Notice::default();
        loop {
            let quiet = (Instant::now() + QUIET).min(limit);
            tokio::select! {
                event = self.receiver.recv() => match event {
                    Some(event) => notice.merge(self.classify(event)),
                    None => break,
                },
                () = self.wake.notified() => notice.lost = true,
                () = tokio::time::sleep_until(quiet) => break,
                _ = stopped.changed() => break,
            }
        }
        notice.lost |= self.lost.swap(false, Ordering::AcqRel);
        notice
    }

    fn classify(&self, event: notify::Result<Event>) -> Notice {
        match event {
            Ok(event) => Notice {
                reattach: event.paths.iter().any(|path| {
                    event_path(path) == event_path(self.layout.state_dir().as_std_path())
                        || event_path(path)
                            == event_path(self.layout.provenance_dir().as_std_path())
                }),
                lost: event.need_rescan(),
            },
            Err(_) => Notice {
                lost: true,
                reattach: true,
            },
        }
    }
}

fn relevant(event: &Event, layout: &ProvenanceLayout) -> bool {
    event.paths.iter().any(|path| {
        let path = event_path(path);
        path.starts_with(event_path(layout.state_dir().as_std_path()).as_ref())
            || path == event_path(layout.provenance_dir().join("settings.json").as_std_path())
            || path == event_path(layout.provenance_dir().as_std_path())
    })
}

#[cfg(not(windows))]
const fn event_path(path: &std::path::Path) -> std::borrow::Cow<'_, std::path::Path> {
    std::borrow::Cow::Borrowed(path)
}

#[cfg(windows)]
fn event_path(path: &std::path::Path) -> std::borrow::Cow<'_, std::path::Path> {
    if let Some(value) = path.to_str() {
        // Compare drive and UNC paths without their verbatim prefix.
        if let Some(value) = value.strip_prefix(r"\\?\UNC\") {
            return std::borrow::Cow::Owned(std::path::PathBuf::from(format!(r"\\{value}")));
        }
        if let Some(value) = value.strip_prefix(r"\\?\") {
            return std::borrow::Cow::Borrowed(std::path::Path::new(value));
        }
    }
    std::borrow::Cow::Borrowed(path)
}

#[cfg(test)]
#[path = "file_events_tests.rs"]
mod tests;
