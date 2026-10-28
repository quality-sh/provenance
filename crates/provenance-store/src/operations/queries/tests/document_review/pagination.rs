use super::*;
use provenance_core::protocol::read_failure::ReadFailure;
use std::collections::BTreeSet;

async fn read(root: &camino::Utf8Path, cursor: Option<String>) -> anyhow::Result<Value> {
    let answer = queries::read_document(
        Some(root.to_owned()),
        &ScopeId::new("default")?,
        ReadPolicy::default(),
        ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal: false,
            cursor,
            limit: 1,
        },
    )
    .await?;
    Ok(serde_json::to_value(answer.result)?)
}

#[tokio::test]
#[provenance_macros::verifies("rule_requirement_document_canonical_membership", examples)]
#[provenance_macros::verifies("rule_document_entry_has_review_outcome", examples)]
#[provenance_macros::verifies("rule_document_has_review_totals", examples)]
async fn document_pages_keep_membership_and_whole_totals() {
    let (dir, store, scope) = seeded_store();
    create_document_records(&store, &scope);
    let root = root_of(&dir);
    let mut cursor = None;
    let mut found = Vec::new();
    loop {
        let page = read(&root, cursor).await.unwrap();
        assert_eq!(
            page["review_totals"],
            json!({"pending":1,"accepted":0,"rejected":0})
        );
        let entry = &page["entries"][0];
        let id = entry["node"]["id"].as_str().unwrap();
        assert_eq!(
            entry["review"]["outcome"],
            if id == "req_overtime" {
                json!("pending")
            } else {
                Value::Null
            }
        );
        found.push((entry["kind"].as_str().unwrap().to_owned(), id.to_owned()));
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        found.into_iter().collect::<BTreeSet<_>>(),
        [
            ("member", "req_overtime"),
            ("member", "resolution_policy"),
            ("member", "rule_policy"),
            ("member", "topic_policy"),
            ("member", "question_policy"),
            ("member", "boundary_no_backpay"),
            ("reference", "source_policy"),
            ("reference", "domain_payroll"),
        ]
        .map(|(kind, id)| (kind.to_owned(), id.to_owned()))
        .into_iter()
        .collect::<BTreeSet<_>>()
    );
}

#[tokio::test]
#[provenance_macros::verifies("rule_review_partial_reads_remain_explicit", examples)]
#[provenance_macros::verifies("rule_cursor_restarts_on_revision_change", examples)]
async fn document_page_refuses_cursor_after_public_write() {
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let first = read(&root, None).await.unwrap();
    store
        .update_requirement(input::<UpdateRequirementInput>(json!({
            "scope_id":scope, "id":"req_overtime", "statement":"The system pays overtime."
        })))
        .unwrap();
    let error = read(&root, Some(first["next_cursor"].as_str().unwrap().into()))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::CursorRevisionChanged)
    );
}

// Characterization: preserve the observed order while the family reads change.
#[tokio::test]
async fn document_entries_keep_observed_member_reference_order() {
    let (dir, store, scope) = seeded_store();
    create_document_records(&store, &scope);
    let result = page(&root_of(&dir), 50, false).await;
    let found = result["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["kind"].as_str().unwrap().to_owned(),
                entry["node"]["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        found,
        [
            ("member", "req_overtime"),
            ("member", "resolution_policy"),
            ("member", "rule_policy"),
            ("member", "topic_policy"),
            ("member", "question_policy"),
            ("member", "boundary_no_backpay"),
            ("reference", "source_policy"),
            ("reference", "domain_payroll"),
        ]
        .map(|(kind, id)| (kind.to_owned(), id.to_owned()))
    );
}
