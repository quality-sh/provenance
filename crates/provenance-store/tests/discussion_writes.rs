use crate::discussion_support;
use discussion_support::*;
use provenance_core::{protocol::failure::OperationError, threads::DiscussionStatus, ThreadStatus};
use provenance_store::operations::catalog::{
    invoke_typed, PreparedContext, PreparedScope, WriteDiscussion, WriteDiscussionRequest,
};
use serde_json::json;

#[test]
#[provenance_macros::verifies("rule_record_comments_have_separate_reply_threads", examples)]
#[provenance_macros::verifies("rule_reply_threads_resolve_independently", examples)]
fn concerns_resolve_independently_and_reopen_without_record_changes() {
    let (_temp, store) = fixture();
    let before = store.list_requirements(&scope()).unwrap();
    let a = start(&store, "concern_a");
    let b = start(&store, "concern_b");
    assert_ne!(a.discussion_id, b.discussion_id);
    assert_eq!(a.thread_id, b.thread_id);
    let resolved = store
        .write_discussion(status(&a, "resolved"))
        .unwrap()
        .discussion;
    assert_eq!(resolved.status, DiscussionStatus::Resolved);
    assert!(store
        .write_discussion(reply(&resolved, "closed_reply"))
        .is_err());
    let reply_b = store
        .write_discussion(reply(&b, "reply_b"))
        .unwrap()
        .discussion;
    assert_eq!(reply_b.status, DiscussionStatus::Active);
    assert_eq!(
        store.list_threads(&scope()).unwrap()[0].status,
        ThreadStatus::Active
    );
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
    assert_eq!(
        store.list_questions(&scope()).unwrap(),
        [] as [provenance_core::Question; 0]
    );
    assert_eq!(
        store.list_dispositions(&scope()).unwrap(),
        [] as [provenance_core::DispositionRecord; 0]
    );
    let reopened = store
        .write_discussion(status(&resolved, "active"))
        .unwrap()
        .discussion;
    assert!(store
        .write_discussion(reply(&reopened, "after_reopen"))
        .is_ok());
}

#[test]
fn reply_and_resolve_race_has_one_winner() {
    let (_temp, store) = fixture();
    let a = start(&store, "root");
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        let one = threads.spawn(|| {
            barrier.wait();
            store.write_discussion(reply(&a, "reply"))
        });
        let two = threads.spawn(|| {
            barrier.wait();
            store.write_discussion(status(&a, "resolved"))
        });
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(results
            .iter()
            .find_map(|r| r.as_ref().err())
            .unwrap()
            .to_string()
            .contains("version"));
    });
}

#[test]
fn typed_parent_scope_and_discussion_membership_are_checked() {
    let (_temp, store) = fixture();
    let a = start(&store, "root");
    for change in [
        json!({"scope_id":"missing"}),
        json!({"parent":{"node_type":"rule","node_id":"req_a"}}),
        json!({"parent":{"node_type":"requirement","node_id":"missing"}}),
        json!({"declared_by":"wrong"}),
    ] {
        let mut input = serde_json::to_value(reply(&a, "bad")).unwrap();
        input
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(store
            .write_discussion(serde_json::from_value(input).unwrap())
            .is_err());
    }
    let mut bad = serde_json::to_value(reply(&a, "bad_id")).unwrap();
    bad["action"]["discussion_id"] = json!("missing");
    assert!(store
        .write_discussion(serde_json::from_value(bad).unwrap())
        .is_err());
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

/// A native caller of `invoke_typed` skips the external check of `write-discussion`. A status
/// change of that caller with no actor is refused as `required` on `request.actor`, and the
/// stored Discussion does not change.
#[tokio::test]
#[provenance_macros::verifies("rule_write_actor_required", examples)]
async fn a_native_status_change_with_no_actor_is_refused() {
    let (temp, store) = fixture();
    let started = start(&store, "root");
    let discussions = temp
        .path()
        .join(".provenance/state/scopes/default/threads/discussions.jsonl");
    let before = std::fs::read_to_string(&discussions).unwrap();
    let mut request = serde_json::to_value(status(&started, "resolved")).unwrap();
    request.as_object_mut().unwrap().remove("actor");
    let request: WriteDiscussionRequest = serde_json::from_value(request).unwrap();
    let context = PreparedContext::for_scope(PreparedScope {
        repository: repository(camino::Utf8Path::from_path(temp.path()).unwrap()),
        requested_target: "selected".into(),
    });

    let error = invoke_typed::<WriteDiscussion>(context, request)
        .await
        .unwrap_err();

    let OperationError::Handler(error) = error else {
        panic!("expected a write refusal");
    };
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({"kind":"invalid_input","field":"request.actor","reason":"required"})
    );
    assert_eq!(error.status(), 400);
    assert_eq!(std::fs::read_to_string(&discussions).unwrap(), before);
}
