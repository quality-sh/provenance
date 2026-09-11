mod discussion_support;
use discussion_support::*;
use provenance_core::{review::SaveOutcome, threads::DiscussionOrigin};
use provenance_macros::verifies;
use provenance_store::{layout::ProvenanceLayout, review::CreateReviewRequirement};
use serde_json::json;

#[test]
#[verifies("rule_comment_created_record_retains_discussion_origin", examples)]
#[verifies("rule_discussion_outcome_shows_record_change", examples)]
fn creation_and_edits_retain_origin_and_immutable_evidence() {
    let (temp, store) = fixture();
    let a = start(&store, "root");
    let origin = DiscussionOrigin {
        discussion_id: a.discussion_id.clone(),
        thread_id: a.thread_id.clone(),
        message_id: a.message_id.clone().unwrap(),
    };
    let input: CreateReviewRequirement = serde_json::from_value(json!({"request_id":"create", "actor":"ben", "origin":origin,
        "create":{"scope_id":"default", "id":"req_new", "statement":"The system retains evidence.", "status":"discovery", "depends_on":[], "supersedes":[], "origin_thread":origin.thread_id,"origin_message":origin.message_id}})).unwrap();
    let serialized = serde_json::to_value(&input).unwrap();
    let created = store.create_review_requirement(input).unwrap();
    assert_eq!(created.outcome, SaveOutcome::Created);
    assert!(created.before.is_none());
    assert_eq!(created.origin, Some(origin.clone()));
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let snapshot = layout
        .scopes_dir()
        .join("default/review/snapshots")
        .join(format!("{}.json", created.after.id.as_str()));
    let original = std::fs::read(&snapshot).unwrap();
    let record: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert_eq!(record["record"]["origin_message"], json!(origin.message_id));
    let edit = serde_json::from_value(json!({"request_id":"edit", "actor":"ben", "expected_etag":created.etag,
        "update":{"scope_id":"default","id":"req_new","description":"Changed"},"relationships":null})).unwrap();
    let changed = store
        .save_requirement_from_discussion(edit, origin.clone())
        .unwrap();
    assert_eq!(changed.before, Some(created.after.clone()));
    assert_ne!(changed.after, created.after);
    assert_eq!(changed.origin, Some(origin));
    assert_eq!(std::fs::read(&snapshot).unwrap(), original);
    assert_eq!(
        store
            .create_review_requirement(serde_json::from_value(serialized).unwrap())
            .unwrap(),
        created
    );
    let saved = store
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|r| r.id.as_str() == "req_new")
        .unwrap();
    assert_eq!(saved.origin_thread, Some(a.thread_id));
    assert_eq!(saved.origin_message, a.message_id);
}

#[test]
fn mismatched_discussion_message_origin_refuses_without_editing() {
    let (_temp, store) = fixture();
    let a = start(&store, "a");
    let b = start(&store, "b");
    let origin = DiscussionOrigin {
        discussion_id: a.discussion_id,
        thread_id: a.thread_id,
        message_id: b.message_id.unwrap(),
    };
    let before = store.list_requirements(&scope()).unwrap();
    assert!(store
        .save_requirement_from_discussion(save(&store, "bad", json!({"description":"bad"})), origin)
        .is_err());
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
}

#[test]
fn creation_receipt_reports_absence_before_the_requirement_exists() {
    let (_temp, store) = fixture();
    let value = json!({"request_id":"create","actor":"ben","origin":null,"create":{"scope_id":"default","id":"req_new","statement":"The system retains evidence.","status":"discovery","depends_on":[],"supersedes":[]}});
    assert!(store
        .requirement_creation_receipt(serde_json::from_value(value.clone()).unwrap())
        .unwrap()
        .is_none());
    let created = store
        .create_review_requirement(serde_json::from_value(value.clone()).unwrap())
        .unwrap();
    assert_eq!(
        store
            .requirement_creation_receipt(serde_json::from_value(value).unwrap())
            .unwrap(),
        Some(created)
    );
}
