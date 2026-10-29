//! Implementation aid: probe the approved native watcher assumptions before host changes.
#[path = "review_host_support/mod.rs"]
mod review_host_support;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use provenance_store::{
    layout::ProvenanceLayout, publication::publication_guard, repository::Repository,
};
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

fn wait_for(receiver: &mpsc::Receiver<notify::Result<Event>>, path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let event = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("native file event before the probe deadline")
            .expect("native watcher event");
        if !event.kind.is_access() && event.paths.iter().any(|candidate| candidate == path) {
            return;
        }
    }
}

#[tokio::test]
async fn native_events_detect_state_replacement_and_reattached_writes_under_guard() {
    let repo = review_host_support::repository();
    let root = camino::Utf8PathBuf::from_path_buf(repo.path().canonicalize().unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root);
    let (sender, receiver) = mpsc::channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .unwrap();
    watcher
        .watch(
            layout.provenance_dir().as_std_path(),
            RecursiveMode::NonRecursive,
        )
        .unwrap();
    watcher
        .watch(layout.state_dir().as_std_path(), RecursiveMode::Recursive)
        .unwrap();
    let repository = Repository::open(repo.path(), "default").unwrap();
    let guard = publication_guard(&repository).await.unwrap();
    let manifest = std::fs::read(layout.manifest_path()).unwrap();
    let old = layout.provenance_dir().join("probe-old-state");
    std::fs::rename(layout.state_dir(), &old).unwrap();
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(layout.manifest_path(), &manifest).unwrap();
    wait_for(&receiver, layout.state_dir().as_std_path());
    let _ = watcher.unwatch(layout.state_dir().as_std_path());
    watcher
        .watch(layout.state_dir().as_std_path(), RecursiveMode::Recursive)
        .unwrap();
    assert_eq!(std::fs::read(layout.manifest_path()).unwrap(), manifest);
    drop(guard);
    let target = layout.state_dir().join("probe.jsonl");
    std::fs::write(&target, "{}\n").unwrap();
    wait_for(&receiver, target.as_std_path());
}
