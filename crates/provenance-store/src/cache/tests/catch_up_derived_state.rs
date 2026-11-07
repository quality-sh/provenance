//! A family's byte domain covers derived fields.
//!
//! A proposal card's effective promotion state is computed from assertions
//! and dispositions. Each test flips an existing card's effective state
//! through one of those inputs and requires rebuild parity.

use super::super::*;
use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::open;
use super::projection_digest_sensitivity::aggregate_layout;
use provenance_core::ScopeId;

fn remove_shard(
    layout: &crate::layout::ProvenanceLayout,
    family: ProjectionFamily,
    scope: &ScopeId,
) {
    let path = family.shard_path(layout, scope);
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }
}

async fn effective_state(layout: &crate::layout::ProvenanceLayout) -> String {
    let pool = open_cache(layout).await.unwrap();
    let state: String =
        sqlx::query_scalar("SELECT promotion_state FROM proposal_cards WHERE id = 'proposal_base'")
            .fetch_one(pool.pool())
            .await
            .unwrap();
    pool.close().await.unwrap();
    state
}

#[tokio::test]
async fn a_hand_emptied_disposition_shard_moves_the_cards_effective_state() {
    let (_dir, layout, scope) = aggregate_layout();
    materialize_state(&open(&layout)).await.unwrap();
    assert_eq!(effective_state(&layout).await, "deferred");

    let dispositions = ProjectionFamily::Dispositions.shard_path(&layout, &scope);
    std::fs::write(dispositions, "").unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
    assert_eq!(effective_state(&layout).await, "asserted");
}

#[tokio::test]
async fn a_writer_path_disposition_moves_the_cards_effective_state() {
    let (_dir, layout, scope) = aggregate_layout();
    remove_shard(&layout, ProjectionFamily::Dispositions, &scope);
    materialize_state(&open(&layout)).await.unwrap();
    assert_eq!(effective_state(&layout).await, "asserted");

    let store = crate::test_support::store(&layout);
    store
        .create_disposition(crate::state_store::CreateDispositionInput {
            scope_id: scope.clone(),
            id: provenance_core::StableId::new("disposition_rejected").unwrap(),
            proposal_id: provenance_core::StableId::new("proposal_base").unwrap(),
            decision: provenance_core::DispositionDecision::Rejected,
            rationale: "Rejected on review".into(),
            actor: provenance_core::DispositionActor {
                identity_type: provenance_core::IdentityType::Human,
                id: "reviewer".into(),
                name: None,
            },
            canonical_artifact: None,
            external_action: None,
        })
        .unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
    assert_eq!(effective_state(&layout).await, "rejected");
}

#[tokio::test]
async fn an_assertion_write_moves_the_cards_effective_state() {
    let (_dir, layout, scope) = aggregate_layout();
    let assertions = ProjectionFamily::AssertionRecords.shard_path(&layout, &scope);
    let assertion_line = std::fs::read_to_string(&assertions).unwrap();
    let packets = ProjectionFamily::SynthesisPackets.shard_path(&layout, &scope);
    let qualifying_packet = std::fs::read_to_string(&packets).unwrap();
    // A qualifying packet requires an assertion. Start with a packet that
    // suggests nothing.
    let mut silent_packet: serde_json::Value =
        serde_json::from_str(qualifying_packet.trim()).unwrap();
    silent_packet["suggested_artifacts"] = serde_json::json!([]);
    std::fs::write(&packets, format!("{silent_packet}\n")).unwrap();
    remove_shard(&layout, ProjectionFamily::AssertionRecords, &scope);
    remove_shard(&layout, ProjectionFamily::Dispositions, &scope);
    materialize_state(&open(&layout)).await.unwrap();
    assert_eq!(effective_state(&layout).await, "proposed");

    std::fs::write(&packets, qualifying_packet).unwrap();
    std::fs::write(&assertions, assertion_line).unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
    assert_eq!(effective_state(&layout).await, "asserted");
}
