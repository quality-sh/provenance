use super::fixtures::seeded_layout;
use crate::cache::{catch_up_state, materialize_state, open_cache, ProjectionFamily};
use serde_json::json;

fn replace_source_stamp(
    layout: &crate::layout::ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    stamp: serde_json::Value,
) {
    let path = ProjectionFamily::Sources.shard_path(layout, scope);
    let mut source: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
    source["created"] = stamp.clone();
    source["updated"] = stamp;
    std::fs::write(path, serde_json::to_string(&source).unwrap() + "\n").unwrap();
}

#[tokio::test]
async fn catch_up_refreshes_stamp_columns_without_changing_the_semantic_digest() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let before = open_cache(&layout).await.unwrap();
    let digest: String =
        sqlx::query_scalar("SELECT digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
            .fetch_one(before.pool())
            .await
            .unwrap();
    before.close().await.unwrap();

    replace_source_stamp(
        &layout,
        &scope,
        json!({"commit":"a".repeat(40),"at":"2026-09-12T00:00:00Z"}),
    );
    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.revision_committed);
    assert_eq!(report.digest, digest);
    let pool = open_cache(&layout).await.unwrap();
    let (created, updated): (String, String) =
        sqlx::query_as("SELECT created, updated FROM sources WHERE id = 'source_schads'")
            .fetch_one(pool.pool())
            .await
            .unwrap();
    assert_eq!(created, updated);
    assert!(created.contains(&"a".repeat(40)));
}

#[tokio::test]
async fn catch_up_validates_a_malformed_stamp_only_change() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    replace_source_stamp(
        &layout,
        &scope,
        json!({"commit":"not-a-commit","at":"2026-09-12T00:00:00Z"}),
    );

    let error = catch_up_state(&layout).await.unwrap_err().to_string();
    assert!(error.contains("commit"), "{error}");
}
