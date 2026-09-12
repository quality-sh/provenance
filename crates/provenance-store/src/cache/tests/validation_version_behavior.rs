use super::catch_up_behavior::dump_family_tables;
use super::catch_up_serial_behavior::latest_revision;
use super::fixtures::{rewrite_records, seeded_layout};
use crate::cache::{
    catch_up_state, materialize_state, open_cache, unit_digest, unit_stored_digest, Unit,
};
use crate::layout::ProvenanceLayout;
use crate::test_probes;
use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};
use provenance_macros::verifies;

fn add_scope(layout: &ProvenanceLayout) -> ScopeId {
    let second = ScopeId::new("second").unwrap();
    let mut manifest: Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.push(Scope {
        id: second.clone(),
        path_prefix: RepoPathPrefix::new("second"),
    });
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    second
}

async fn accept_invalid_bytes(layout: &ProvenanceLayout, scope: &ScopeId) {
    rewrite_records(&crate::shards::rules_path(layout, scope), |r| {
        r["requirement_ids"] = serde_json::json!([]);
    });
    let pool = open_cache(layout).await.unwrap();
    let unit = Unit::Scope(scope.clone());
    sqlx::query("UPDATE projection_unit_digests SET digest = ?, stored_digest = ? WHERE unit = ?")
        .bind(unit_digest(&layout.state_dir(), &unit).unwrap())
        .bind(unit_stored_digest(&layout.state_dir(), &unit).unwrap())
        .bind(format!("scope:{}", scope.as_str()))
        .execute(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
}

#[tokio::test]
#[verifies("rule_catch_up_validates_changed_units_only", examples)]
async fn an_unchanged_invalid_scope_does_not_refuse_a_read() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    accept_invalid_bytes(&layout, &scope).await;
    let result = catch_up_state(&layout).await;
    assert!(result.is_ok(), "unchanged scope refused: {result:?}");
    assert!(!result.unwrap().revision_committed);
    assert!(materialize_state(&layout).await.is_err());
}

#[tokio::test]
#[verifies("rule_catch_up_validates_changed_units_only", examples)]
async fn a_changed_scope_alone_is_validated() {
    let (_dir, layout, scope) = seeded_layout();
    let second = add_scope(&layout);
    materialize_state(&layout).await.unwrap();
    accept_invalid_bytes(&layout, &scope).await;
    let path = crate::shards::rules_path(&layout, &second);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "").unwrap();
    test_probes::start_recording_reads();
    let result = catch_up_state(&layout).await;
    let reads = test_probes::take_recorded_reads();
    assert!(result.is_ok(), "unchanged scope refused: {result:?}");
    assert!(reads.contains(path.as_str()));
    assert!(!reads.contains(crate::shards::rules_path(&layout, &scope).as_str()));
    std::fs::write(&path, "invalid JSON\n").unwrap();
    assert!(catch_up_state(&layout).await.is_err());
}

#[tokio::test]
#[verifies("rule_catch_up_validates_changed_units_only", examples)]
async fn a_manifest_change_validates_every_scope() {
    let (_dir, layout, scope) = seeded_layout();
    add_scope(&layout);
    materialize_state(&layout).await.unwrap();
    accept_invalid_bytes(&layout, &scope).await;
    let pool = open_cache(&layout).await.unwrap();
    let before = latest_revision(pool.pool()).await;
    let rows = dump_family_tables(pool.pool()).await;
    pool.close().await.unwrap();
    let mut bytes = std::fs::read(layout.manifest_path()).unwrap();
    bytes.push(b'\n');
    std::fs::write(layout.manifest_path(), bytes).unwrap();
    let error = catch_up_state(&layout).await.unwrap_err();
    assert!(
        error.to_string().contains("needs one requirement"),
        "{error}"
    );
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(latest_revision(pool.pool()).await, before);
    assert_eq!(dump_family_tables(pool.pool()).await, rows);
    pool.close().await.unwrap();
}

pub async fn rewind_validation(pool: &sqlx::SqlitePool) {
    sqlx::query("INSERT OR REPLACE INTO projection_validation VALUES (1, 0)")
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
#[verifies("rule_validation_version_move_rebuilds_the_projection", examples)]
async fn a_validation_version_move_routes_catch_up_to_a_full_rebuild() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let before = latest_revision(pool.pool()).await;
    rewind_validation(pool.pool()).await;
    sqlx::query("DELETE FROM requirements")
        .execute(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
    let report = catch_up_state(&layout).await.unwrap();
    assert!(
        report.rebuilt,
        "old validation version did not rebuild: {report:?}"
    );
    assert_eq!(report.serial, before.0 + 1);
    let pool = open_cache(&layout).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requirements")
        .fetch_one(pool.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    pool.close().await.unwrap();
    assert!(!catch_up_state(&layout).await.unwrap().revision_committed);
}

#[tokio::test]
async fn a_validation_version_move_refuses_an_unchanged_invalid_scope() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    accept_invalid_bytes(&layout, &scope).await;
    let pool = open_cache(&layout).await.unwrap();
    rewind_validation(pool.pool()).await;
    let before = latest_revision(pool.pool()).await;
    pool.close().await.unwrap();
    assert!(catch_up_state(&layout).await.is_err());
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(latest_revision(pool.pool()).await, before);
    let version: i64 = sqlx::query_scalar("SELECT version FROM projection_validation")
        .fetch_one(pool.pool())
        .await
        .unwrap();
    assert_eq!(version, 0);
    pool.close().await.unwrap();
}
