use super::subscribers::Subscribers;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use provenance_store::{
    layout::ProvenanceLayout, publication::publication_guard, review_changes::ReviewStateSnapshot,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::{mpsc, watch, Notify};

pub(super) struct Observation {
    layout: ProvenanceLayout,
    watcher: Arc<Mutex<RecommendedWatcher>>,
    receiver: mpsc::Receiver<notify::Result<Event>>,
    lost: Arc<AtomicBool>,
    wake: Arc<Notify>,
}

#[derive(Default)]
struct Notice {
    reattach: bool,
    lost: bool,
}

impl Observation {
    pub fn new(layout: ProvenanceLayout) -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::channel(256);
        let lost = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Notify::new());
        let callback_layout = layout.clone();
        let callback_lost = lost.clone();
        let callback_wake = wake.clone();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            if let Ok(event) = &event {
                if !event.need_rescan()
                    && (event.kind.is_access() || !relevant(event, &callback_layout))
                {
                    return;
                }
            }
            if sender.try_send(event).is_err() {
                callback_lost.store(true, Ordering::Release);
                callback_wake.notify_one();
            }
        })?;
        watcher.watch(
            layout.provenance_dir().as_std_path(),
            RecursiveMode::NonRecursive,
        )?;
        watcher.watch(layout.state_dir().as_std_path(), RecursiveMode::Recursive)?;
        Ok(Self {
            layout,
            watcher: Arc::new(Mutex::new(watcher)),
            receiver,
            lost,
            wake,
        })
    }

    pub async fn read(&self, reattach: bool) -> anyhow::Result<ReviewStateSnapshot> {
        let guard = publication_guard(&self.layout).await?;
        let watcher = self.watcher.clone();
        let layout = self.layout.clone();
        tokio::task::spawn_blocking(move || {
            if reattach {
                let mut watcher = watcher.lock().unwrap();
                let _ = watcher.unwatch(layout.state_dir().as_std_path());
                watcher.watch(layout.state_dir().as_std_path(), RecursiveMode::Recursive)?;
            }
            ReviewStateSnapshot::read(&guard)
        })
        .await?
    }

    pub async fn run(
        mut self,
        mut baseline: Option<ReviewStateSnapshot>,
        subscribers: Subscribers,
    ) {
        let mut stopped = subscribers.stopped();
        loop {
            let Some(notice) = self.next(&mut stopped).await else {
                break;
            };
            if notice.lost {
                baseline = None;
                subscribers.unavailable("file change notifications were lost");
                continue;
            }
            // Finish a started guarded read before shutdown joins the observer.
            let read = self.read(notice.reattach).await;
            match read {
                Ok(current) => {
                    if let Some(previous) = &baseline {
                        subscribers.changed(current.changes_since(previous));
                    } else {
                        subscribers.reset("observation-recovered");
                    }
                    baseline = Some(current);
                    subscribers.available();
                }
                Err(error) => {
                    baseline = None;
                    subscribers.unavailable(&format!("{error:#}"));
                }
            }
        }
    }

    async fn next(&mut self, stopped: &mut watch::Receiver<bool>) -> Option<Notice> {
        if *stopped.borrow() {
            return None;
        }
        let first = tokio::select! {
            event = self.receiver.recv() => event?,
            _ = self.wake.notified() => {
                return Some(Notice { lost: self.lost.swap(false, Ordering::AcqRel), reattach: true });
            }
            _ = stopped.changed() => return None,
        };
        let mut notice = self.classify(first);
        while let Ok(event) = self.receiver.try_recv() {
            let next = self.classify(event);
            notice.reattach |= next.reattach;
            notice.lost |= next.lost;
        }
        notice.lost |= self.lost.swap(false, Ordering::AcqRel);
        Some(notice)
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

fn event_path(path: &std::path::Path) -> std::borrow::Cow<'_, std::path::Path> {
    #[cfg(windows)]
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
