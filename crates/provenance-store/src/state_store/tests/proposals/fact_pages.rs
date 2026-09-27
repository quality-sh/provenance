//! The proposal fact pages list only the facts filed against the named
//! proposal.

use super::disposition_write_gate::{allow_actor, rejected_input};
use super::{super::initialized_store, proposal_input};
use provenance_core::PromotionState;
use serde_json::{json, Value};

async fn fact_page(root: &camino::Utf8Path, operation: &str, proposal_id: &str) -> Value {
    super::super::read_budget::operation_read(
        root,
        operation,
        json!({"proposal_id": proposal_id, "limit": 10, "cursor": null}),
    )
    .await
    .unwrap()
}

fn items(page: &Value) -> &Vec<Value> {
    page["result"]["items"].as_array().unwrap()
}

#[tokio::test]
async fn a_fact_page_lists_the_facts_of_the_named_proposal_only() {
    let (dir, store, scope) = initialized_store();
    allow_actor(&store);
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    store
        .create_disposition(rejected_input(
            &scope,
            "disposition_overtime",
            "proposal_overtime",
        ))
        .unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

    let dispositions = fact_page(&root, "page-proposal-dispositions-v2", "proposal_overtime").await;
    assert_eq!(items(&dispositions).len(), 1);
    assert_eq!(items(&dispositions)[0]["id"], "disposition_overtime");
    assert_eq!(dispositions["result"]["has_more"], false);

    let other = fact_page(&root, "page-proposal-dispositions-v2", "proposal_leave").await;
    assert!(items(&other).is_empty());

    let assertions = fact_page(&root, "page-proposal-assertions-v2", "proposal_overtime").await;
    assert!(items(&assertions).is_empty());
}
