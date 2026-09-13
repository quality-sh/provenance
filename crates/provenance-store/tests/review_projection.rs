mod review_support;
use provenance_store::{cache, layout::ProvenanceLayout};
use review_support::*;
use serde_json::json;

#[tokio::test]
async fn receipt_only_save_changes_projection_and_rebuild_keeps_history() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    cache::materialize_state(&layout).await.unwrap();
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", layout.cache_db_path()))
        .await
        .unwrap();
    let before: String =
        sqlx::query_scalar("SELECT digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    store
        .save_requirement(save(&store, "noop", json!({})))
        .unwrap();
    cache::materialize_state(&layout).await.unwrap();
    let after: String =
        sqlx::query_scalar("SELECT digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(
        before, after,
        "receipt changes must participate in projection freshness"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM review_journal")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    pool.close().await;
}
