//! A family's byte domain covers derived fields.
//!
//! A proposal card's effective promotion state is computed from assertions,
//! dispositions, and legacy promotion decisions. Each test flips an existing
//! card's effective state through one of those inputs and requires rebuild
//! parity.

use super::super::*;
use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::projection_digest_sensitivity::aggregate_layout;
use crate::state_store::StateStore;
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
            .fetch_one(&pool)
            .await
            .unwrap();
    pool.close().await;
    state
}

#[tokio::test]
async fn a_hand_emptied_disposition_shard_moves_the_cards_effective_state() {
    let (_dir, layout, scope) = aggregate_layout();
    materialize_state(&layout).await.unwrap();
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
    materialize_state(&layout).await.unwrap();
    assert_eq!(effective_state(&layout).await, "asserted");

    let store = StateStore::new(layout.clone());
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
    materialize_state(&layout).await.unwrap();
    assert_eq!(effective_state(&layout).await, "proposed");

    std::fs::write(&packets, qualifying_packet).unwrap();
    std::fs::write(&assertions, assertion_line).unwrap();

    assert_catch_up_equals_rebuild(&layout).await;
    assert_eq!(effective_state(&layout).await, "asserted");
}

#[tokio::test]
async fn a_legacy_promotion_decision_is_refused_by_catch_up_and_by_rebuild() {
    // The promotion_decisions shard is frozen at the shipped-v1 audit. A
    // rebuild refuses a new legacy decision. Catch-up must refuse it the same
    // way and commit nothing.
    let (_dir, layout, scope) = aggregate_layout();
    remove_shard(&layout, ProjectionFamily::Dispositions, &scope);
    materialize_state(&layout).await.unwrap();
    assert_eq!(effective_state(&layout).await, "asserted");
    let pool = open_cache(&layout).await.unwrap();
    let serial_before: i64 = sqlx::query_scalar("SELECT MAX(serial) FROM projection_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;

    let legacy = crate::shards::legacy_promotion_decisions_path(&layout, &scope);
    std::fs::write(
        legacy,
        format!(
            "{}\n",
            serde_json::json!({"schema_version": 1, "scope_id": scope.as_str(),
                "promotionDecisionId": "decision_legacy", "proposalId": "proposal_base",
                "decision": "accepted", "rationale": "Accepted.",
                "decidedBy": {"identityType": "human", "userId": "reviewer"},
                "canonicalArtifact": {"artifactType": "requirement",
                    "artifactId": "req_schads_overtime"}})
        ),
    )
    .unwrap();

    let catch_up_error = catch_up_state(&layout).await.unwrap_err().to_string();
    let rebuild_error = materialize_state(&layout).await.unwrap_err().to_string();
    assert!(
        catch_up_error.contains("frozen shipped-v1"),
        "{catch_up_error}"
    );
    assert_eq!(catch_up_error, rebuild_error, "refusal parity");
    assert_eq!(
        effective_state(&layout).await,
        "asserted",
        "nothing committed"
    );
    let pool = open_cache(&layout).await.unwrap();
    let serial_after: i64 = sqlx::query_scalar("SELECT MAX(serial) FROM projection_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    assert_eq!(serial_after, serial_before);
}
