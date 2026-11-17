use super::{file_events::FileEvents, subscribers::Subscribers};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use provenance_store::{
    cache::ProjectionFamily,
    layout::ProvenanceLayout,
    publication::publication_guard,
    repository::Repository,
    review_changes::{ReviewStateCapture, ReviewStateSnapshot},
    state_store::StateStore,
};
use std::sync::{Arc, Mutex};

pub(super) struct Observation {
    repository: Repository,
    layout: ProvenanceLayout,
    watcher: Arc<Mutex<RecommendedWatcher>>,
    events: FileEvents,
}

impl Observation {
    pub fn new(repository: Repository) -> anyhow::Result<Self> {
        let layout = repository.layout().clone();
        let (handler, events) = FileEvents::new(layout.clone());
        let mut watcher = notify::recommended_watcher(handler)?;
        watcher.watch(
            layout.provenance_dir().as_std_path(),
            RecursiveMode::NonRecursive,
        )?;
        watcher.watch(layout.state_dir().as_std_path(), RecursiveMode::Recursive)?;
        Ok(Self {
            repository,
            layout,
            watcher: Arc::new(Mutex::new(watcher)),
            events,
        })
    }

    /// Reads one snapshot. The guard is held while the bytes are read. The
    /// records parse after the guard is released, so the first read after a
    /// write does not wait for this parse. When the parse fails, the snapshot
    /// is read again under the guard. So the subscribers get the error of a
    /// guarded read.
    pub async fn read(&self, reattach: bool) -> anyhow::Result<ReviewStateSnapshot> {
        let capture = self.capture(reattach).await?;
        let parsed =
            tokio::task::spawn_blocking(move || ReviewStateSnapshot::from_capture(capture)).await?;
        if let Ok(snapshot) = parsed {
            return Ok(snapshot);
        }
        let guard = publication_guard(&self.repository).await?;
        tokio::task::spawn_blocking(move || ReviewStateSnapshot::read(&guard)).await?
    }

    async fn capture(&self, reattach: bool) -> anyhow::Result<ReviewStateCapture> {
        let guard = publication_guard(&self.repository).await?;
        let watcher = self.watcher.clone();
        let layout = self.layout.clone();
        tokio::task::spawn_blocking(move || {
            if reattach {
                let mut watcher = watcher.lock().unwrap();
                let _ = watcher.unwatch(layout.state_dir().as_std_path());
                let _ = watcher.unwatch(layout.provenance_dir().as_std_path());
                watcher.watch(
                    layout.provenance_dir().as_std_path(),
                    RecursiveMode::NonRecursive,
                )?;
                watcher.watch(layout.state_dir().as_std_path(), RecursiveMode::Recursive)?;
            }
            require_regular_file(layout.manifest_path().as_std_path())?;
            require_regular_file(layout.provenance_dir().join("settings.json").as_std_path())?;
            let manifest = StateStore::under_guard(&guard).manifest()?;
            for scope in &manifest.scopes {
                for family in ProjectionFamily::ALL {
                    require_regular_file(
                        layout
                            .scopes_dir()
                            .join(scope.id.as_str())
                            .join(family.shard_suffix())
                            .as_std_path(),
                    )?;
                }
            }
            ReviewStateSnapshot::capture(&guard)
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
            let Some(notice) = self.events.next(&mut stopped).await else {
                break;
            };
            if notice.lost {
                baseline = None;
            }
            // Finish a started guarded read before shutdown joins the observer.
            let first = self.read(notice.reattach || notice.lost).await;
            let mut reset_reason = notice.lost.then_some("file-events-lost");
            let read = if let Ok(current) = first {
                Ok(current)
            } else {
                baseline = None;
                if self.events.settle(&mut stopped).await.lost {
                    reset_reason = Some("file-events-lost");
                }
                reset_reason.get_or_insert("observation-recovered");
                self.read(true).await
            };
            match read {
                Ok(current) => {
                    if let Some(reason) = reset_reason {
                        subscribers.reset(reason);
                    } else if let Some(previous) = &baseline {
                        subscribers.changed(current.changes_since(previous));
                    } else {
                        subscribers.reset("observation-recovered");
                    }
                    baseline = Some(current);
                    subscribers.available();
                }
                Err(error) => {
                    baseline = None;
                    subscribers.unavailable("observation-failed", &format!("{error:#}"));
                }
            }
        }
    }
}

fn require_regular_file(path: &std::path::Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => anyhow::ensure!(
            metadata.is_file(),
            "state path is not a regular file: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
