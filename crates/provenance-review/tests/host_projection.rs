//! The review host prepares the projection before it reports that it is ready, so the
//! first page read has no projection work to do.

use provenance_macros::verifies;
use provenance_porcelain::check::{Category, CheckPort, PortFuture};
use provenance_review::{HostOptions, RepositoryScope};
use provenance_store::cache::{catch_up_state, CatchUpReport};
use provenance_transport::fixture::records::Repository;
use std::{path::Path, sync::Arc};

/// A check port for a host whose test never runs a check.
struct NoChecks;

impl CheckPort for NoChecks {
    fn run<'a>(&'a self, _: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async { Err("this host runs no checks".to_owned()) })
    }
}

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_review_host_prepares_projection_at_start", examples)]
async fn a_started_host_leaves_no_projection_build_for_the_first_read() {
    let cache = tempfile::tempdir().unwrap();
    use_user_cache(cache.path());

    // A new worktree has no projection database.
    let fresh = Repository::new("The review host builds the projection at start.");
    remove_projection(fresh.dir.path());
    assert_eq!(pass_after_start(&fresh).await, (false, false));

    // A projection from before a state change is out of date.
    let changed = Repository::new("The review host reads the changed state.");
    catch_up_state(&changed.open()).await.unwrap();
    changed.edit("default", "The review host reads the edited state.");
    assert_eq!(pass_after_start(&changed).await, (false, false));
}

/// Starts a host, then reports whether one catch-up pass still had to rebuild or commit.
async fn pass_after_start(repository: &Repository) -> (bool, bool) {
    let place = RepositoryScope::new(repository.dir.path().to_str().unwrap(), "default");
    let host = provenance_review::start(HostOptions {
        place,
        repository_id: "local".to_owned(),
        port: 0,
        check_port: Arc::new(NoChecks),
        on_read_only: Box::new(|| {}),
    })
    .await
    .unwrap();
    let CatchUpReport {
        rebuilt,
        revision_committed,
        ..
    } = catch_up_state(&repository.open()).await.unwrap();
    drop(host);
    (rebuilt, revision_committed)
}

fn remove_projection(root: &Path) {
    let cache = root.join(".provenance").join("cache");
    if cache.exists() {
        std::fs::remove_dir_all(cache).unwrap();
    }
}

/// Points every platform's per-user cache at one isolated directory.
fn use_user_cache(root: &Path) {
    let cache = root.join("cache");
    std::env::set_var("HOME", root);
    std::env::set_var("XDG_CACHE_HOME", &cache);
    std::env::set_var("LOCALAPPDATA", &cache);
}
