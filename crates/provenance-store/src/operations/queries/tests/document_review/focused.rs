use super::*;

#[tokio::test]
#[provenance_macros::verifies("rule_content_change_requires_new_review", examples)]
async fn document_revision_clears_the_prior_outcome_for_each_reviewable_kind() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    let records = create_document_records(&store, &scope);
    let root = root_of(&dir);

    for (kind, id) in records {
        let proposal = if kind == NodeType::Requirement {
            store
                .record_decision_state(&scope, kind, &id)
                .unwrap()
                .pending
                .unwrap()
                .proposal_id
        } else {
            submit(&store, &scope, kind, &id)
        };
        decide(
            &store,
            &scope,
            kind,
            &id,
            proposal,
            DispositionDecision::Accepted,
        );
        revise(&store, kind, &id);

        let revised = page(&root, 50, false).await;
        let expected = if kind == NodeType::Requirement {
            json!("pending")
        } else {
            Value::Null
        };
        assert_eq!(review(&revised, id.as_str())["outcome"], expected);
    }
}

#[tokio::test]
#[provenance_macros::verifies("rule_record_details_load_on_open", examples)]
async fn document_comment_count_summarizes_feedback_without_loading_comments() {
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

    let document = page(&root_of(&dir), 50, false).await;
    let summary = review(&document, id.as_str());
    assert_eq!(summary["comment_count"], 1);
    assert_eq!(
        summary
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        ["comment_count", "outcome", "pending_proposal_id"]
            .into_iter()
            .collect()
    );
}
