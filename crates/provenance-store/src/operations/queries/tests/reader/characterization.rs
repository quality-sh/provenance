//! What a read does to the read database file today, under each freshness
//! policy. These tests have no Rule. They hold the file behaviour while the
//! read database moves behind a port, so the move changes no answer, no
//! refusal text and no file beside the database.

use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_core::protocol::StampPolicy;

fn side_files(store: &TestStore) -> Vec<String> {
    let database = store.layout().cache_db_path();
    ["-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| format!("{database}{suffix}"))
        .filter(|path| std::path::Path::new(path).exists())
        .collect()
}

#[tokio::test]
async fn each_policy_answers_at_one_serial_and_leaves_no_side_files() {
    let store = test_stores::seeded_queries();
    assert!(!store.layout().cache_db_path().exists());

    let caught_up = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert!(caught_up.result.found);
    assert_eq!(caught_up.stamp.policy, StampPolicy::CatchUp);
    assert!(caught_up.freshness_error.is_none());
    assert!(
        store.layout().cache_db_path().exists(),
        "catch-up creates it"
    );
    assert_eq!(side_files(&store), Vec::<String>::new());

    for (policy, word) in [
        (FreshnessPolicy::AnnotateOnly, StampPolicy::AnnotateOnly),
        (FreshnessPolicy::RefuseStale, StampPolicy::RefuseStale),
        (FreshnessPolicy::CatchUp, StampPolicy::CatchUp),
    ] {
        let stamped = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap();
        assert_eq!(stamped.stamp.policy, word);
        assert_eq!(stamped.stamp.serial, caught_up.stamp.serial);
        assert_eq!(stamped.stamp.digest, caught_up.stamp.digest);
        assert_eq!(
            serde_json::to_value(&stamped.result).unwrap(),
            serde_json::to_value(&caught_up.result).unwrap()
        );
        assert_eq!(side_files(&store), Vec::<String>::new(), "{word:?}");
    }
}

#[tokio::test]
async fn a_read_with_no_database_names_the_file_and_creates_nothing() {
    let store = test_stores::seeded_queries();
    let database = store.layout().cache_db_path();
    for policy in [FreshnessPolicy::AnnotateOnly, FreshnessPolicy::RefuseStale] {
        let refused = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap_err()
            .to_string();
        let expected =
            format!("no projection revision in {database}; run `provenance materialize` (");
        assert!(refused.starts_with(&expected), "{policy:?}: {refused}");
        assert!(!database.exists(), "{policy:?}");
    }
}

#[tokio::test]
async fn a_stale_projection_refuses_with_the_file_and_the_moved_unit() {
    let store = test_stores::seeded_queries();
    let caught_up = get_through(&store, ReadPolicy::default()).await.unwrap();
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        "req_after_the_read",
        provenance_core::RequirementStatus::Active,
    );
    let refused = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::RefuseStale),
    )
    .await
    .unwrap_err()
    .to_string();
    let expected = format!(
        "refuse_stale: the projection in {} at serial {} (digest {}, instance ",
        store.layout().cache_db_path(),
        caught_up.stamp.serial,
        caught_up.stamp.digest
    );
    assert!(refused.starts_with(&expected), "{refused}");
    assert!(refused.contains("; moved: "), "{refused}");
    assert_eq!(side_files(&store), Vec::<String>::new());
}
