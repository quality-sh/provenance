mod discussion_support;
use discussion_support::{fixture, scope};
use provenance_core::threads::DiscussionStatus;
use provenance_store::{
    layout::ProvenanceLayout,
    review::{TargetDiscussionAction, TargetDiscussionWrite},
    state_store::StateStore,
    write_error::{WriteError, WriteFailure},
};
use serde_json::json;

fn request(id: &str, action: serde_json::Value) -> TargetDiscussionWrite {
    serde_json::from_value(json!({
        "scope_id": "default", "request_id": id, "actor": "ben",
        "declared_by": null, "action": action
    }))
    .unwrap()
}

fn start(id: &str) -> TargetDiscussionWrite {
    request(id, json!({"kind":"start","parent":{"node_type":"requirement","node_id":"req_a"},"role":"user","body":id}))
}

fn reply(id: &str, discussion: &provenance_core::threads::DiscussionEntry) -> TargetDiscussionWrite {
    request(id, json!({"kind":"reply","discussion_id":discussion.discussion_id,"expected_version":discussion.version,"role":"user","body":id}))
}

fn failure(result: anyhow::Result<provenance_core::threads::DiscussionEntry>) -> WriteFailure {
    WriteError(result.unwrap_err()).safe()
}

#[test]
fn target_reply_changes_only_its_discussion() {
    let (_temp, store) = fixture();
    let a = store.write_target_discussion(start("a")).unwrap();
    let b = store.write_target_discussion(start("b")).unwrap();
    let changed = store.write_target_discussion(reply("reply_a", &a)).unwrap();
    assert_eq!(changed.discussion_id, a.discussion_id);
    assert_eq!(changed.version, 2);
    assert_eq!(b.version, 1);
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 3);
}

#[test]
fn target_refusals_do_not_append_messages() {
    let (_temp, store) = fixture();
    let a = store.write_target_discussion(start("a")).unwrap();
    assert!(matches!(failure(store.write_target_discussion(request("missing", json!({"kind":"reply","discussion_id":"missing","expected_version":1,"role":"user","body":"x"})))), WriteFailure::ResourceNotFound));
    assert!(matches!(failure(store.write_target_discussion(request("unsupported", json!({"kind":"start","parent":{"node_type":"domain","node_id":"req_a"},"role":"user","body":"x"})))), WriteFailure::UnsupportedThreadParent));
    assert!(matches!(failure(store.write_target_discussion(request("parent_missing", json!({"kind":"start","parent":{"node_type":"requirement","node_id":"missing"},"role":"user","body":"x"})))), WriteFailure::ResourceNotFound));
    assert!(matches!(failure(store.write_target_discussion(request("stale", json!({"kind":"reply","discussion_id":a.discussion_id,"expected_version":0,"role":"user","body":"x"})))), WriteFailure::DiscussionVersionConflict));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[test]
fn replay_after_restart_returns_receipt_and_changed_intent_refuses() {
    let (temp, store) = fixture();
    let first = store.write_target_discussion(start("a")).unwrap();
    let store = StateStore::new(ProvenanceLayout::new(
        camino::Utf8Path::from_path(temp.path()).unwrap(),
    ));
    assert_eq!(store.write_target_discussion(start("a")).unwrap(), first);
    let mut changed = start("a");
    if let TargetDiscussionAction::Start { body, .. } = &mut changed.action {
        *body = "changed".into();
    }
    assert!(matches!(failure(store.write_target_discussion(changed)), WriteFailure::DiscussionIntentChanged));
    let first_reply = store.write_target_discussion(reply("r1", &first)).unwrap();
    let wrong_target = request("r1", json!({"kind":"reply","discussion_id":"missing","expected_version":first.version,"role":"user","body":"r1"}));
    assert!(matches!(failure(store.write_target_discussion(wrong_target)), WriteFailure::DiscussionIntentChanged));
    assert_eq!(first_reply.version, 2);
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
}

#[test]
fn one_expected_version_wins_concurrent_replies() {
    let (_temp, store) = fixture();
    let a = store.write_target_discussion(start("a")).unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        let one = threads.spawn(|| { barrier.wait(); store.write_target_discussion(reply("r1", &a)) });
        let two = threads.spawn(|| { barrier.wait(); store.write_target_discussion(reply("r2", &a)) });
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(results.into_iter().filter(Result::is_err).any(|result| matches!(failure(result), WriteFailure::DiscussionVersionConflict)));
    });
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
}

#[test]
fn resolved_discussion_refuses_target_reply() {
    let (_temp, store) = fixture();
    let a = store.write_target_discussion(start("a")).unwrap();
    let resolved = store.write_discussion(discussion_support::status(&a, "resolve", "resolved")).unwrap();
    assert_eq!(resolved.status, DiscussionStatus::Resolved);
    assert!(matches!(failure(store.write_target_discussion(reply("reply", &resolved))), WriteFailure::DiscussionResolved));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}
