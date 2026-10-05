mod review_support;
use provenance_store::{cache, layout::ProvenanceLayout};
use review_support::*;
use serde_json::json;

async fn revision_digest(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_scalar("SELECT digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Implementation aid: pins that an equal save publishes no change; no Rule
/// names it.
#[tokio::test]
async fn equal_save_leaves_the_projection_unchanged() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    cache::materialize_state(&layout).await.unwrap();
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", layout.cache_db_path()))
        .await
        .unwrap();
    let before = revision_digest(&pool).await;
    store
        .save_requirement(save(&store, "noop", json!({})))
        .unwrap();
    cache::materialize_state(&layout).await.unwrap();
    assert_eq!(revision_digest(&pool).await, before);
    pool.close().await;
}
