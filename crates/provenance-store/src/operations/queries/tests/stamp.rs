//! The stamp names the stored projection instance and revision.

use crate::cache::tests::fixtures::seeded_layout;
use crate::cache::{materialize_state, open_cache};
use crate::operations::stamp;
use provenance_core::protocol::StampPolicy;

#[tokio::test]
async fn the_stamp_carries_the_stored_instance_id() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let instance_id: String = sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (serial, digest): (i64, String) = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut connection = pool.acquire().await.unwrap();
    let stored = stamp::stored_revision(&mut connection)
        .await
        .unwrap()
        .expect("a materialized database holds a revision");
    let stamp = stored.stamp(StampPolicy::CatchUp, Vec::new(), vec!["canonical".into()]);

    assert_eq!(stamp.instance_id, instance_id);
    assert_eq!(stamp.serial, serial);
    assert_eq!(stamp.digest, digest);
}

#[tokio::test]
async fn an_empty_database_holds_no_revision() {
    let (_dir, layout, _scope) = seeded_layout();
    crate::cache::materialize_empty_state(&layout)
        .await
        .unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    assert!(stamp::stored_revision(&mut connection)
        .await
        .unwrap()
        .is_none());
}
