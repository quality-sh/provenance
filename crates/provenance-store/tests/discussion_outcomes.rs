mod discussion_support;
use discussion_support::*;
use provenance_core::{
    review::{EvidenceQuery, ReviewHistoryQuery, SaveOutcome},
    threads::DiscussionOrigin,
    NodeType,
};
use provenance_macros::verifies;
use provenance_store::{
    operations::read_policy::ReadPolicy,
    review::{read_evidence, read_history, CreateReviewRequirement},
};
use serde_json::json;

fn origin_of(discussion: &provenance_core::threads::Discussion) -> DiscussionOrigin {
    DiscussionOrigin {
        discussion_id: discussion.discussion_id.clone(),
        thread_id: discussion.thread_id.clone(),
        message_id: discussion.root_message_id.clone(),
    }
}

fn create_from(origin: &DiscussionOrigin) -> CreateReviewRequirement {
    serde_json::from_value(
        json!({"request_id":"create", "actor":"ben", "origin":origin,
        "create":{"scope_id":"default", "id":"req_new", "statement":"The system retains evidence.",
            "status":"discovery", "depends_on":[], "supersedes":[],
            "origin_thread":origin.thread_id,"origin_message":origin.message_id}}),
    )
    .unwrap()
}

fn req_new() -> provenance_core::StableId {
    provenance_core::StableId::new("req_new").unwrap()
}

#[test]
#[verifies("rule_comment_created_record_retains_discussion_origin", examples)]
fn comment_created_record_retains_its_origin() {
    let (_temp, store) = fixture();
    let a = start(&store, "root");
    let origin = origin_of(&a);
    let input = create_from(&origin);
    let serialized = serde_json::to_value(&input).unwrap();
    let created = store.create_review_requirement(input).unwrap();
    assert_eq!(created.outcome, SaveOutcome::Created);
    assert_eq!(created.origin, Some(origin));
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
        .find(|r| r.id == req_new())
        .unwrap();
    assert_eq!(saved.origin_thread, Some(a.thread_id));
    assert_eq!(saved.origin_message, Some(a.root_message_id));
}

#[tokio::test]
#[verifies("rule_discussion_outcome_shows_record_change", examples)]
async fn discussion_edit_version_shows_its_change() {
    let (temp, store) = fixture();
    let a = start(&store, "root");
    let origin = origin_of(&a);
    let created = store
        .create_review_requirement(create_from(&origin))
        .unwrap();
    commit_state(&temp, "Create from the comment");
    let edit = serde_json::from_value(json!({"request_id":"edit", "actor":"ben", "expected_etag":created.etag,
        "update":{"scope_id":"default","id":"req_new","statement":"The system retains changed evidence."},
        "relationships":null}))
    .unwrap();
    store
        .save_requirement_from_discussion(edit, origin.clone())
        .unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let history = read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            record_kind: NodeType::Requirement,
            record_id: req_new(),
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap()
    .result
    .entries;
    let changed = history.last().unwrap();
    assert_eq!(changed.id.as_str(), "working");
    assert_eq!(changed.origin, Some(origin));
    assert_eq!(changed.changed_fields, ["statement"]);
    let statement = |before: bool| {
        let query = EvidenceQuery {
            record_kind: NodeType::Requirement,
            record_id: req_new(),
            entry_id: changed.id.clone(),
            before,
            field: Some("statement".into()),
            offset: 0,
        };
        async move {
            read_evidence(root, &scope(), ReadPolicy::default(), query)
                .await
                .unwrap()
                .result
                .json_text
        }
    };
    assert_eq!(statement(true).await, "\"The system retains evidence.\"");
    assert_eq!(
        statement(false).await,
        "\"The system retains changed evidence.\""
    );
}

#[test]
fn mismatched_discussion_message_origin_refuses_without_editing() {
    let (_temp, store) = fixture();
    let a = start(&store, "a");
    let b = start(&store, "b");
    let origin = DiscussionOrigin {
        discussion_id: a.discussion_id,
        thread_id: a.thread_id,
        message_id: b.root_message_id,
    };
    let before = store.list_requirements(&scope()).unwrap();
    assert!(store
        .save_requirement_from_discussion(save(&store, "bad", json!({"description":"bad"})), origin)
        .is_err());
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
}

#[test]
fn repeated_guarded_creation_resolves_to_the_recorded_outcome() {
    let (_temp, store) = fixture();
    let value = json!({"request_id":"create","actor":"ben","origin":null,"create":{"scope_id":"default","id":"req_new","statement":"The system retains evidence.","status":"discovery","depends_on":[],"supersedes":[]}});
    let created = store
        .create_review_requirement(serde_json::from_value(value.clone()).unwrap())
        .unwrap();
    assert_eq!(
        store
            .create_review_requirement(serde_json::from_value(value).unwrap())
            .unwrap(),
        created
    );
    assert_eq!(store.list_requirements(&scope()).unwrap().len(), 2);
}
