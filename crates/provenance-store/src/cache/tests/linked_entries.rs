//! A symbolic link at a name that canonical state can use refuses: the
//! view of the local adapter, the catch-up and the hash with no lock all
//! give the same refusal, and the projection keeps its rows.

use super::catch_up_behavior::dump_family_tables;
use super::fixtures::{open, seeded_layout};
use crate::cache::{catch_up_state, materialize_state, open_cache, unit_digest, Unit};
use crate::storage::{local::LocalStateStorage, StateStorage};
use camino::Utf8PathBuf;
use provenance_macros::verifies;

/// Moves the entry at `path` out of the state directory and puts a link to
/// it at `path`, then checks that each reader refuses the link with
/// `unsupported state entry: <path>` and that the rows stand.
async fn assert_link_refuses(
    layout: &crate::layout::ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    path: &Utf8PathBuf,
) {
    let repository = open(layout);
    materialize_state(&repository).await.unwrap();
    let pool = open_cache(layout).await.unwrap();
    let before = dump_family_tables(pool.pool()).await;
    pool.close().await.unwrap();

    let target = layout.root().join("moved-out-of-state");
    std::fs::rename(path, &target).unwrap();
    std::os::unix::fs::symlink(&target, path).unwrap();
    let expected = format!("unsupported state entry: {path}");

    let view = LocalStateStorage::new(layout.clone())
        .read_view()
        .unwrap_err();
    assert_eq!(format!("{view:#}"), expected);
    let error = catch_up_state(&repository).await.unwrap_err();
    assert!(format!("{error:#}").contains(&expected), "{error:#}");
    let unlocked = unit_digest(&layout.state_dir(), &Unit::Scope(scope.clone())).unwrap_err();
    assert_eq!(unlocked.path, *path);
    assert_eq!(format!("{:#}", unlocked.error), expected);

    let pool = open_cache(layout).await.unwrap();
    assert_eq!(dump_family_tables(pool.pool()).await, before);
    pool.close().await.unwrap();
}

#[tokio::test]
#[verifies("rule_catch_up_reads_units_from_the_view", examples)]
async fn a_link_at_a_shard_name_refuses() {
    let (_dir, layout, scope) = seeded_layout();
    let path = crate::shards::requirements_path(&layout, &scope);
    assert_link_refuses(&layout, &scope, &path).await;
}

#[tokio::test]
#[verifies("rule_catch_up_reads_units_from_the_view", examples)]
async fn a_link_at_a_shard_directory_refuses() {
    let (_dir, layout, scope) = seeded_layout();
    let shard = crate::shards::requirements_path(&layout, &scope);
    let path = shard.parent().unwrap().to_path_buf();
    assert_link_refuses(&layout, &scope, &path).await;
}

#[tokio::test]
#[verifies("rule_catch_up_reads_units_from_the_view", examples)]
async fn a_link_at_a_scope_directory_refuses() {
    let (_dir, layout, scope) = seeded_layout();
    let path = layout.scopes_dir().join(scope.as_str());
    assert_link_refuses(&layout, &scope, &path).await;
}
