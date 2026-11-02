//! A catch-up pass frees the publication lock before its cache connection
//! closes, so a writer does not wait on the close.

use camino::Utf8Path;
use fs2::FileExt;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_store::cache::catch_up_state;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::publication_guard;
use provenance_store::repository::Repository;

/// Implementation aid: pins that `catch_up_state` drops the publication
/// guard before the connection close; no Rule names it.
#[tokio::test(flavor = "multi_thread")]
async fn a_writer_takes_the_lock_while_the_catch_up_close_waits() {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(dir.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest =
        Manifest::default_with_scope(ScopeId::new("default").unwrap(), RepoPathPrefix::new("."));
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let repository = Repository::open(dir.path(), "default").unwrap();

    // Another close holds the close lock, so the pass cannot finish its close.
    let database = repository.layout().cache_db_path();
    std::fs::create_dir_all(database.parent().unwrap()).unwrap();
    let close_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(database.with_extension("db.close.lock"))
        .unwrap();
    close_lock.lock_exclusive().unwrap();

    let pass = tokio::spawn({
        let repository = repository.clone();
        async move { catch_up_state(&repository).await }
    });
    // The pass creates the database under the guard.
    while !database.exists() {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // A writer gets the lock while the close still waits. If the pass held
    // the guard over the close, the writer would wait until the close
    // reached its deadline, and the pass would report that failure.
    drop(publication_guard(&repository).await.unwrap());
    fs2::FileExt::unlock(&close_lock).unwrap();
    let report = pass.await.unwrap().expect("the close completes");
    assert!(report.rebuilt);
}
