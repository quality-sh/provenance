#[allow(dead_code)]
mod review_support;
use provenance_core::review::{EvidenceQuery, ReviewHistoryQuery};
use provenance_store::{
    operations::read_policy::ReadPolicy,
    review::{read_evidence, read_history, SaveRequirement},
};
use review_support::*;
use serde_json::json;

fn query(limit: usize, cursor: Option<String>) -> ReviewHistoryQuery {
    ReviewHistoryQuery {
        record_kind: provenance_core::NodeType::Requirement,
        record_id: id(),
        limit,
        cursor,
    }
}

#[tokio::test]
async fn history_pages_follow_versions_one_at_a_time() {
    let (temp, store) = fixture();
    let created = commit_state(&temp, "Create");
    store
        .save_requirement(save(&store, json!({"description":"Changed"})))
        .unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let complete = read_history(root, &scope(), ReadPolicy::default(), query(200, None))
        .await
        .unwrap();
    assert_eq!(complete.result.entries.len(), 2);

    let mut cursor = None;
    let mut versions = Vec::new();
    loop {
        let page = read_history(root, &scope(), ReadPolicy::default(), query(1, cursor))
            .await
            .unwrap();
        versions.extend(
            page.result
                .entries
                .iter()
                .map(|version| version.id.as_str().to_owned()),
        );
        cursor = page.result.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(versions, [created.as_str(), "working"]);
}

#[tokio::test]
async fn evidence_reassembles_exact_unicode_and_history_cursor_is_bound() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    commit_state(&temp, "Create");
    let text = "😀\n\"é\\".repeat(25_000);
    store
        .save_requirement(save(&store, json!({"description":text})))
        .unwrap();
    let described = commit_state(&temp, "Describe");
    store
        .save_requirement(save(&store, json!({"description":"later"})))
        .unwrap();
    let mut offset = 0;
    let mut bytes = String::new();
    loop {
        let page = read_evidence(
            root,
            &scope(),
            ReadPolicy::default(),
            EvidenceQuery {
                record_kind: provenance_core::NodeType::Requirement,
                record_id: id(),
                entry_id: provenance_core::StableId::new(described.clone()).unwrap(),
                before: false,
                field: None,
                offset,
            },
        )
        .await
        .unwrap();
        assert!(serde_json::to_vec(&page.result).unwrap().len() <= 65_536);
        assert_eq!(page.result.version.as_str(), described);
        bytes.push_str(&page.result.json_text);
        match page.result.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    let record: provenance_core::Requirement = serde_json::from_str(&bytes).unwrap();
    assert_eq!(record.description.as_deref(), Some(text.as_str()));
    let one = read_history(root, &scope(), ReadPolicy::default(), query(1, None))
        .await
        .unwrap();
    assert_eq!(one.result.entries.len(), 1);
    let cursor = one.result.next_cursor.unwrap();
    let two = read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        query(1, Some(cursor)),
    )
    .await
    .unwrap();
    assert_eq!(two.result.entries[0].id.as_str(), described);
    let cursor = two.result.next_cursor.unwrap();
    let three = read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        query(1, Some(cursor.clone())),
    )
    .await
    .unwrap();
    assert_eq!(three.result.entries[0].id.as_str(), "working");
    assert!(three.result.next_cursor.is_none());
    store
        .save_requirement(save(&store, json!({"description":"again"})))
        .unwrap();
    assert!(read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        query(1, Some(cursor))
    )
    .await
    .is_err());
    assert!(
        read_history(root, &scope(), ReadPolicy::default(), query(201, None))
            .await
            .is_err()
    );
}

#[test]
fn saves_stay_authoritative_after_reopen() {
    let (temp, store) = fixture();
    let first = store.save_requirement(save(&store, json!({}))).unwrap();
    drop(store);
    let store = provenance_store::state_store::StateStore::new(
        provenance_store::layout::ProvenanceLayout::new(
            camino::Utf8Path::from_path(temp.path()).unwrap(),
        ),
    );
    let repeat: SaveRequirement =
        serde_json::from_value(serde_json::to_value(save(&store, json!({}))).unwrap()).unwrap();
    assert_eq!(store.save_requirement(repeat).unwrap(), first);
    assert!(store
        .save_requirement(save(&store, json!({"description":"new"})))
        .is_ok());
}

/// Implementation aid: pins the 8 KiB evidence page split; no Rule names
/// paging.
#[tokio::test]
async fn evidence_pages_a_long_field_from_git() {
    let (temp, store) = fixture();
    let text = "é\n".repeat(20_000);
    store
        .save_requirement(save(&store, json!({"description":text})))
        .unwrap();
    let described = commit_state(&temp, "Describe");
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let mut offset = 0;
    let mut encoded = String::new();
    loop {
        let query = serde_json::from_value(json!({"requirement_id":"req_a", "entry_id":described,
            "before":false,"offset":offset,"field":"description"}))
        .unwrap();
        let page = read_evidence(root, &scope(), ReadPolicy::default(), query)
            .await
            .unwrap();
        assert!(page.result.json_text.len() <= 8192);
        encoded.push_str(&page.result.json_text);
        match page.result.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(serde_json::from_str::<String>(&encoded).unwrap(), text);
}
