//! The byte domain of a family is every file its readers read.
//!
//! Messages span month shards, edges span every shard in the edges
//! directory, and five ideation families overlay records from the landings
//! shard (dispositions also read legacy promotion decisions). A catch-up
//! that hashes one file per family misses all of them; these tests pin the
//! complete domains against a fresh rebuild.

use super::super::*;
use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::*;
use crate::state_store::{IdeationLandingBatch, StateStore};

#[tokio::test]
async fn a_landed_ideation_batch_reaches_the_projection_through_catch_up() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    let store = StateStore::new(layout.clone());
    let batch: IdeationLandingBatch = serde_json::from_value(serde_json::json!({
        "contributions": [{
            "schema_version": 1, "scope_id": scope.as_str(), "id": "contribution_landed",
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
            serde_json::json!({"schema_version": 1, "scope_id": scope.as_str(),
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
            serde_json::json!({"schema_version": 1, "scope_id": scope.as_str(),
                "id": "message_late", "thread_id": "thread_a", "role": "user",
                "body": "Late month", "created_at": 2})
        ),
    )
    .unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_second_edge_shard_is_covered_by_the_sweep() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    let edges_dir = crate::shards::edges_path(&layout)
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::create_dir_all(&edges_dir).unwrap();
    std::fs::write(
        edges_dir.join("edges-01.jsonl"),
        format!(
            "{}\n",
            serde_json::json!({"schema_version": 1, "scope_id": scope.as_str(),
                "id": "edge_second_shard", "edge_type": "references",
                "from_type": "requirement", "from_id": "req_schads_overtime",
                "to_type": "source", "to_id": "source_schads"})
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

    // After the rebuild snapshots canonical state, an out-of-band writer
    // rewrites the live shard. The stored baseline must describe the bytes
    // the rows were derived from, so the next pass sees the difference.
    crate::test_probes::arm("stamp_before_baselines", move || {
        std::fs::write(&live_path, &edited).unwrap();
        Ok(())
    });
    materialize_state(&layout).await.unwrap();
    crate::test_probes::disarm("stamp_before_baselines");

    assert_catch_up_equals_rebuild(&layout).await;
}
