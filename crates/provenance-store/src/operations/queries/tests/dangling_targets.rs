//! A contribution written before the write-time target check can name a
//! record that is gone. Freshness annotates and never refuses, so a read
//! over that state still answers with a stamp and no freshness error.

use super::{root_of, seeded_store};
use crate::cache::tests::fixtures::append_record;
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::GetQuery;
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::verifies;
use serde_json::json;

#[tokio::test]
#[verifies("rule_old_dangling_ideation_target_is_a_gap", examples)]
async fn a_read_over_an_old_dangling_target_still_answers() {
    let (dir, _store, scope) = seeded_store();
    let layout = ProvenanceLayout::new(root_of(&dir));
    append_record(
        &crate::shards::contributions_path(&layout, &scope),
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "contrib_old",
            "target": {"artifact_type": "requirement", "artifact_id": "req_gone"},
            "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
            "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
            "challenges": [], "suggested_artifact_changes": [], "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
        }),
    );

    let answer = crate::operations::queries::get(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        GetQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            node_type: NodeType::Requirement,
            id: "req_overtime".into(),
        },
    )
    .await
    .expect("the read answers");
    assert!(
        answer.freshness_error.is_none(),
        "freshness annotates and never refuses: {:?}",
        answer.freshness_error
    );
    assert!(answer.result.found, "the requirement is still served");
    assert!(answer.stamp.serial >= 1, "the answer carries a stamp");
}
