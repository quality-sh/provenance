//! A repository opened through a directory link reads its state under the
//! resolved root, and the cache that the store opens holds what it read.

use super::super::connection::cache_options;
use super::fixtures::{rewrite_records, seeded_layout};
use crate::cache::{catch_up_state, materialize_state};
use crate::repository::Repository;
use crate::test_probes;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;

#[cfg(unix)]
fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

/// A directory junction. Unlike a directory symbolic link, a junction does
/// not need administrator rights or Developer Mode.
#[cfg(windows)]
fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed: {status}");
}

#[tokio::test]
#[verifies("rule_opened_root_is_resolved_working_tree", examples)]
async fn a_repository_opened_through_a_link_reads_its_state_under_the_resolved_root() {
    let (dir, layout, scope) = seeded_layout();
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("repository");
    link_dir(dir.path(), &link);
    let resolved = Utf8PathBuf::from_path_buf(dunce::canonicalize(dir.path()).unwrap()).unwrap();

    let repository = Repository::open(&link, "default").unwrap();
    assert_eq!(repository.working_tree(), resolved);
    assert!(!repository.working_tree().as_str().starts_with(r"\\?\"));
    // The Windows runner's TEMP has the short name `RUNNER~1`. The opened
    // root names the long form.
    #[cfg(windows)]
    {
        if std::env::var_os("GITHUB_ACTIONS").is_some() {
            assert!(link.to_str().unwrap().contains('~'), "{link:?}");
        }
        assert!(!repository.working_tree().as_str().contains('~'));
    }

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

    let pool = sqlx::SqlitePool::connect_with(cache_options(repository.layout()))
        .await
        .unwrap();
    let statement: String = sqlx::query_scalar("SELECT statement FROM requirements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(statement, "Changed through the link");
    pool.close().await;
}
