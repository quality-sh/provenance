//! The byte domain of a family is every file its readers read.
//!
//! Messages span month shards. Edges span every shard in the edges
//! directory. Five ideation families overlay records from the landings
//! shard. These tests pin the complete domains against a fresh rebuild.

use super::super::*;
use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::*;
use super::projection_digest_sensitivity::{aggregate_layout, change_one_record};
use crate::state_store::{IdeationLandingBatch, StateStore};
use provenance_core::SUPPORTED_SCHEMA_VERSION;

/// Every family gets its own invalidation between materialize and catch-up,
/// and each must equal a fresh rebuild.
#[tokio::test]
async fn every_family_invalidation_reaches_the_projection() {
    for family in ProjectionFamily::ALL {
        println!("invalidating family `{}`", family.family_name());
        let (_dir, layout, scope) = aggregate_layout();
        materialize_state(&layout).await.unwrap();
        change_one_record(&layout, family, &scope);
        assert_catch_up_equals_rebuild(&layout).await;
    }
}

#[tokio::test]
async fn a_second_scope_is_swept_and_stays_equivalent() {
    let (_dir, layout, first_scope) = seeded_layout();
    let second_scope = provenance_core::ScopeId::new("second").unwrap();
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.push(provenance_core::Scope {
        id: second_scope.clone(),
        path_prefix: provenance_core::RepoPathPrefix::new("second"),
    });
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let requirements = crate::shards::requirements_path(&layout, &second_scope);
    std::fs::create_dir_all(requirements.parent().unwrap()).unwrap();
    std::fs::write(
        &requirements,
        format!(
            "{}\n",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "second",
                "id": "req_second", "statement": "Second", "status": "active"})
        ),
    )
    .unwrap();
    materialize_state(&layout).await.unwrap();

    // Change one record in each scope. Both must reach the projection.
    let rules = crate::shards::rules_path(&layout, &first_scope);
    let edited = std::fs::read_to_string(&rules)
        .unwrap()
        .replace("Pay overtime", "Pay double overtime");
    std::fs::write(&rules, edited).unwrap();
    std::fs::write(
        &requirements,
        format!(
            "{}\n",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "second",
                "id": "req_second", "statement": "Second, restated", "status": "active"})
        ),
    )
    .unwrap();
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_landed_ideation_batch_reaches_the_projection_through_catch_up() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    let store = StateStore::new(layout.clone());
    let batch: IdeationLandingBatch = serde_json::from_value(serde_json::json!({
        "contributions": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(), "id": "contribution_landed",
            "target": {"artifact_type": "requirement", "artifact_id": "req_schads_overtime"},
            "participant_slot": "slot_a", "stance": "support",
            "strongest_finding": "Landed", "evidence_references": [], "material_claims": [],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "R"}, "open_questions": []
        }],
        "synthesis_packets": [], "proposals": [], "assertions": [], "dispositions": []
    }))
    .unwrap();
    store.land_ideation_batch(&scope, batch, false).unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_second_month_message_shard_is_covered_by_the_sweep() {
    let (_dir, layout, scope) = seeded_layout();
    let threads_dir = crate::shards::threads_path(&layout, &scope)
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::create_dir_all(&threads_dir).unwrap();
    std::fs::write(
        crate::shards::threads_path(&layout, &scope),
        format!(
            "{}\n",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(),
                "id": "thread_a",
                "parent": {"node_type": "requirement", "node_id": "req_schads_overtime"},
                "status": "active", "created_at": 1})
        ),
    )
    .unwrap();
    materialize_state(&layout).await.unwrap();

    std::fs::write(
        threads_dir.join("2026-08.jsonl"),
        format!(
            "{}\n",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(),
                "id": "message_late", "thread_id": "thread_a", "role": "user",
                "body": "Late month", "created_at": 2})
        ),
    )
    .unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_live_edit_racing_the_rebuild_baseline_is_caught_by_the_next_catch_up() {
    let (_dir, layout, scope) = seeded_layout();
    let live_path = crate::shards::requirements_path(&layout, &scope);
    let edited = std::fs::read_to_string(&live_path)
        .unwrap()
        .replace("Overtime", "Edited between snapshot and stamp");
    assert_ne!(edited, std::fs::read_to_string(&live_path).unwrap());

    // After the rebuild snapshots canonical state, a writer rewrites the
    // live shard. The stored digests must describe the bytes the rows came
    // from, so the next pass sees the difference.
    crate::test_probes::arm("stamp_before_unit_digests", move || {
        std::fs::write(&live_path, &edited).unwrap();
        Ok(())
    });
    materialize_state(&layout).await.unwrap();
    crate::test_probes::disarm("stamp_before_unit_digests");

    assert_catch_up_equals_rebuild(&layout).await;
}
