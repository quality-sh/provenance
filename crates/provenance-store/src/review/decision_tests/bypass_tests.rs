//! Native and batch writers must not reach around the review seam.

use super::{enrolled, refused};
use serde_json::json;

#[test]
fn native_and_batch_writers_refuse_review_mutation_bypasses() {
    let (_temp, store, _) = enrolled();
    let card = json!({
        "scope_id":"default","id":"prop-forge","proposal_key":"forge","proposal_type":"record_revision",
        "title":"T","summary":"S","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_a"},
        "source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"builds_on":[],
        "promotion_state":"proposed","record_revision":{"revision":"rev","content_digest":"digest"}}
    );
    refused(
        store.create_proposal_card(serde_json::from_value(card.clone()).unwrap()),
        "review submissions go through the review seam",
    );
    refused(
        store.create_disposition(
            serde_json::from_value(json!({
            "scope_id":"default","id":"disp-forge","proposal_id":"prop-1","decision":"rejected",
            "rationale":"Because","actor":{"identity_type":"human","id":"reviewer"}}))
            .unwrap(),
        ),
        "decisions on review submissions go through the review seam",
    );

    let mut batch_card = card;
    batch_card["schema_version"] = json!(3);
    refused(
        store.land_ideation_batch(
            &super::scope(),
            serde_json::from_value(json!({"proposals":[batch_card]})).unwrap(),
            false,
        ),
        "batch writes cannot create review submissions",
    );
    refused(
        store.land_ideation_batch(&super::scope(), serde_json::from_value(json!({"dispositions":[{
            "schema_version":2,"scope_id":"default","id":"disp-forge","proposal_id":"prop-1",
            "decision":"rejected","rationale":"Because","actor":{"identity_type":"human","id":"reviewer"}}]})).unwrap(), false),
        "decisions on review submissions go through the review seam",
    );
}
