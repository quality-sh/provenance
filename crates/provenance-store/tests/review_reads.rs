mod review_support;
use provenance_core::review::{EvidenceQuery, RequirementSnapshot, ReviewHistoryQuery};
use provenance_store::{
    operations::read_policy::ReadPolicy,
    review::{read_evidence, read_history},
};
use review_support::*;
use serde_json::json;

#[tokio::test]
async fn evidence_reassembles_exact_unicode_and_history_cursor_is_bound() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let text = "😀\n\"é\\".repeat(25_000);
    let first = store
        .save_requirement(save(&store, "enroll", json!({"description":text})))
        .unwrap();
    store
        .save_requirement(save(&store, "next", json!({"description":"later"})))
        .unwrap();
    let mut offset = 0;
    let mut bytes = String::new();
    loop {
        let page = read_evidence(
            root,
            &scope(),
            ReadPolicy::default(),
            EvidenceQuery {
                requirement_id: id(),
                entry_id: first.id.clone(),
                before: false,
                field: None,
                offset,
            },
        )
        .await
        .unwrap();
        assert!(serde_json::to_vec(&page.result).unwrap().len() <= 65_536);
        assert_eq!(page.result.snapshot, first.after);
        bytes.push_str(&page.result.json_text);
        match page.result.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(
        provenance_store::canonical_digest::digest(bytes.as_bytes()),
        first.after.digest
    );
    let snapshot: RequirementSnapshot = serde_json::from_str(&bytes).unwrap();
    assert_eq!(snapshot.record.description.as_deref(), Some(text.as_str()));
    let one = read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            requirement_id: id(),
            limit: 1,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(one.result.entries, [first]);
    let cursor = one.result.next_cursor.unwrap();
    let two = read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            requirement_id: id(),
            limit: 1,
            cursor: Some(cursor.clone()),
        },
    )
    .await
    .unwrap();
    assert_eq!(two.result.entries[0].request_id.as_str(), "next");
    store
        .save_requirement(save(&store, "noop", json!({})))
        .unwrap();
    assert!(read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            requirement_id: id(),
            limit: 1,
            cursor: Some(cursor)
        }
    )
    .await
    .is_err());
    assert!(read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            requirement_id: id(),
            limit: 201,
            cursor: None
        }
    )
    .await
    .is_err());
}

#[test]
fn receipts_are_authoritative_after_reopen_and_private_to_the_actor() {
    let (temp, store) = fixture();
    let first = store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    drop(store);
    let store = provenance_store::state_store::StateStore::new(
        provenance_store::layout::ProvenanceLayout::new(
            camino::Utf8Path::from_path(temp.path()).unwrap(),
        ),
    );
    assert_eq!(
        store
            .requirement_save_receipt(&scope(), &id(), &first.request_id, "ben", None)
            .unwrap(),
        Some(first)
    );
    assert!(store
        .requirement_save_receipt(
            &scope(),
            &id(),
            &provenance_core::StableId::new("missing").unwrap(),
            "ben",
            None
        )
        .unwrap()
        .is_none());
    assert!(store
        .requirement_save_receipt(
            &scope(),
            &id(),
            &provenance_core::StableId::new("enroll").unwrap(),
            "other",
            None
        )
        .is_err());
}

#[tokio::test]
async fn field_evidence_preserves_long_text_without_loading_other_fields() {
    let (temp, store) = fixture();
    let text = "é\n".repeat(20_000);
    let first = store
        .save_requirement(save(&store, "field", json!({"description":text})))
        .unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let mut offset = 0;
    let mut encoded = String::new();
    loop {
        let query = serde_json::from_value(json!({"requirement_id":"req_a", "entry_id":first.id,
            "before":false,"offset":offset,"field":"description"}))
        .unwrap();
        let page = read_evidence(root, &scope(), ReadPolicy::default(), query)
            .await
            .unwrap();
        encoded.push_str(&page.result.json_text);
        match page.result.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(serde_json::from_str::<String>(&encoded).unwrap(), text);
}
