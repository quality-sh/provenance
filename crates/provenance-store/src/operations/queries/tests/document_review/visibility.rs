use super::*;

#[tokio::test]
#[provenance_macros::verifies("rule_review_document_hides_rejected_new_records", examples)]
#[provenance_macros::verifies("rule_document_has_review_totals", examples)]
async fn document_filter_hides_rejected_new_records_and_keeps_accepted_records() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    create_document_records(&store, &scope);
    let id = sid("rule_policy");
    let proposal = submit(&store, &scope, NodeType::Rule, &id);
    decide(
        &store,
        &scope,
        NodeType::Rule,
        &id,
        proposal,
        DispositionDecision::Rejected,
    );
    let root = root_of(&dir);
    let full = page(&root, 50, false).await;
    assert_eq!(review(&full, "rule_policy")["outcome"], "rejected");
    assert_eq!(
        full["review_totals"],
        json!({"pending":1,"accepted":0,"rejected":1})
    );
    let filtered = page(&root, 50, true).await;
    assert!(!filtered["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["node"]["id"] == "rule_policy"));
    assert_eq!(
        filtered["review_totals"],
        json!({"pending":1,"accepted":0,"rejected":0})
    );
    let id = sid("resolution_policy");
    let proposal = submit(&store, &scope, NodeType::Resolution, &id);
    decide(
        &store,
        &scope,
        NodeType::Resolution,
        &id,
        proposal,
        DispositionDecision::Accepted,
    );
    revise(&store, NodeType::Resolution, &id);
    let proposal = submit(&store, &scope, NodeType::Resolution, &id);
    decide(
        &store,
        &scope,
        NodeType::Resolution,
        &id,
        proposal,
        DispositionDecision::Rejected,
    );
    let filtered = page(&root, 50, true).await;
    assert_eq!(
        review(&filtered, "resolution_policy")["outcome"],
        "rejected"
    );
    assert_eq!(
        filtered["review_totals"],
        json!({"pending":1,"accepted":0,"rejected":1})
    );
}

#[tokio::test]
#[provenance_macros::verifies("rule_document_entry_has_review_outcome", examples)]
#[provenance_macros::verifies("rule_document_has_review_totals", examples)]
async fn document_distinguishes_decisions_within_each_family() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    create_document_records(&store, &scope);
    store
        .create_rule(input::<CreateRuleInput>(json!({
            "scope_id":scope, "id":"rule_second", "statement":"The system checks the policy.",
            "status":"draft", "severity":"medium", "requirement_ids":["req_overtime"],
            "resolution_ids":[]
        })))
        .unwrap();
    store
        .create_resolution(input::<CreateResolutionInput>(json!({
            "scope_id":scope, "id":"resolution_second", "title":"Second decision",
            "position":"Check the policy.", "rationale":"The policy needs a check.",
            "status":"draft", "requirement_ids":["req_overtime"], "supersedes":[], "inputs":[]
        })))
        .unwrap();
    for (kind, accepted, rejected) in [
        (NodeType::Rule, "rule_policy", "rule_second"),
        (
            NodeType::Resolution,
            "resolution_policy",
            "resolution_second",
        ),
    ] {
        for (id, decision) in [
            (accepted, DispositionDecision::Accepted),
            (rejected, DispositionDecision::Rejected),
        ] {
            let id = sid(id);
            let proposal = submit(&store, &scope, kind, &id);
            decide(&store, &scope, kind, &id, proposal, decision);
        }
    }
    let result = page(&root_of(&dir), 50, false).await;
    for id in ["rule_policy", "resolution_policy"] {
        assert_eq!(review(&result, id)["outcome"], "accepted", "{id}");
    }
    for id in ["rule_second", "resolution_second"] {
        assert_eq!(review(&result, id)["outcome"], "rejected", "{id}");
    }
    assert_eq!(
        result["review_totals"],
        json!({"pending":1,"accepted":2,"rejected":2})
    );
}
