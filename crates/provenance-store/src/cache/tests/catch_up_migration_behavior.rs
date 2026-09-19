//! A migration that recreates tables commits before the rebuild that
//! refills them. A crash in between must not leave the tables empty at a
//! serial that claims them full: the next pass reloads every family.

use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::catch_up_serial_behavior::latest_revision;
use super::fixtures::{append_record, seeded_layout, sid};
use crate::cache::{catch_up_state, materialize_state, open_cache};
use crate::migrations::{
    RECORD_COLUMNS_MIGRATION_ID, RECORD_DELETION_MIGRATION_ID, RESOURCE_PAYLOADS_MIGRATION_ID,
};
use crate::state_store::{PostMessageInput, StateStore};
use provenance_core::{MessageRole, NodeType, ThreadParent, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::verifies;

async fn requirement_count(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM requirements")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Forgetting that 022 ran makes the next pass apply it again, which drops
/// and recreates the eleven tables: the shape of a migration over a live
/// database.
async fn forget_migration_022(pool: &sqlx::SqlitePool) {
    sqlx::query("DELETE FROM _schema_migrations WHERE id IN (?, ?, ?)")
        .bind(RECORD_COLUMNS_MIGRATION_ID)
        .bind(RECORD_DELETION_MIGRATION_ID)
        .bind(crate::migrations::RECORD_STAMPS_MIGRATION_ID)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
#[verifies("rule_interrupted_migration_reloads_every_family", examples)]
async fn a_crash_between_a_migration_and_its_rebuild_is_healed_by_the_next_pass() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (serial_before, _) = latest_revision(pool.pool()).await;
    forget_migration_022(pool.pool()).await;
    pool.close().await.unwrap();

    crate::test_probes::crash_at("catch_up_after_migrations");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("catch_up_after_migrations");
    assert!(error.to_string().contains("injected crash"), "{error}");

    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(
        requirement_count(pool.pool()).await,
        0,
        "the migration committed and emptied the table before the crash"
    );
    pool.close().await.unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(requirement_count(pool.pool()).await, 1, "{report:?}");
    let (serial_after, _) = latest_revision(pool.pool()).await;
    assert!(serial_after > serial_before, "{report:?}");
    pool.close().await.unwrap();
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn resource_payload_migration_rebuilds_unchanged_collaboration_families() {
    let (_dir, layout, scope) = seeded_layout();
    let store = StateStore::new(layout.clone());
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: sid("req_schads_overtime"),
            },
            role: MessageRole::User,
            body: "Review this requirement.".into(),
        })
        .unwrap();
    append_record(
        &crate::shards::proposal_cards_path(&layout, &scope),
        &serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "proposal_migration",
            "proposal_key": "migration",
            "proposal_type": "requirement_candidate",
            "title": "Migration payload",
            "summary": "Keep this exact proposal.",
            "traceability": {
                "target": {
                    "artifact_type": "requirement",
                    "artifact_id": "req_schads_overtime"
                },
                "source_ids": [],
                "evidence_references": [],
                "supporting_claim_ids": []
            },
            "promotion_state": "proposed"
        }),
    );
    append_record(
        &crate::shards::dispositions_path(&layout, &scope),
        &serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "disposition_migration",
            "proposal_id": "proposal_migration",
            "decision": "rejected",
            "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": "reviewer"}
        }),
    );
    materialize_state(&layout).await.unwrap();

    let pool = open_cache(&layout).await.unwrap();
    for table in ["threads", "messages", "proposal_cards", "dispositions"] {
        sqlx::query(&format!("ALTER TABLE {table} DROP COLUMN payload"))
            .execute(pool.pool())
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM _schema_migrations WHERE id = ?")
        .bind(RESOURCE_PAYLOADS_MIGRATION_ID)
        .execute(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.migrations_applied, [RESOURCE_PAYLOADS_MIGRATION_ID]);
    let pool = open_cache(&layout).await.unwrap();
    for (table, id) in [
        ("threads", "thread_requirement_req_schads_overtime"),
        ("messages", "msg_000001"),
        ("proposal_cards", "proposal_migration"),
        ("dispositions", "disposition_migration"),
    ] {
        let payload: String = sqlx::query_scalar(&format!(
            "SELECT payload FROM {table} WHERE scope_id = ? AND id = ?"
        ))
        .bind(scope.as_str())
        .bind(id)
        .fetch_one(pool.pool())
        .await
        .unwrap();
        let record: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(record["id"], id);
    }
    pool.close().await.unwrap();
}
