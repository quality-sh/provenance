//! A repository opened through a link reads and caches under its resolved root.

use super::fixtures::{rewrite_records, seeded_layout};
use crate::cache::{catch_up_state, materialize_state};
use crate::repository::Repository;
use crate::test_probes;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;
use sqlx::sqlite::SqliteConnectOptions;
use std::str::FromStr as _;

#[cfg(unix)]
fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

#[tokio::test]
#[verifies("rule_operation_acts_on_opened_repository", examples)]
async fn a_repository_opened_through_a_link_reads_and_caches_under_its_resolved_root() {
    let (dir, layout, scope) = seeded_layout();
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("repository");
    link_dir(dir.path(), &link);
    let resolved = Utf8PathBuf::from_path_buf(dunce::canonicalize(dir.path()).unwrap()).unwrap();

    let repository = Repository::open(&link, "default").unwrap();
    assert_eq!(repository.working_tree(), resolved);
    // On Windows the root has no verbatim prefix, so a user reads the plain
    // path and a connection URL does not take `?` as the query.
    assert!(!repository.working_tree().as_str().starts_with(r"\\?\"));

    materialize_state(&repository).await.unwrap();
    rewrite_records(&crate::shards::requirements_path(&layout, &scope), |r| {
        r["statement"] = "Changed through the link".into();
    });
    test_probes::start_recording_reads();
    catch_up_state(&repository).await.unwrap();
    let reads = test_probes::take_recorded_reads();
    assert!(!reads.is_empty());
    let state = resolved.join(".provenance").join("state");
    for path in reads {
        assert!(
            Utf8Path::new(&path).starts_with(&state),
            "read outside the resolved state: {path}"
        );
    }

    let url = format!("sqlite://{}", repository.layout().cache_db_path());
    let pool = sqlx::SqlitePool::connect_with(SqliteConnectOptions::from_str(&url).unwrap())
        .await
        .unwrap();
    let statement: String = sqlx::query_scalar("SELECT statement FROM requirements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(statement, "Changed through the link");
    pool.close().await;
}
