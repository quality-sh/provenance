mod discussion_support;
use discussion_support::*;
use provenance_core::{threads::DiscussionStatus, ThreadStatus};
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
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
        .write_discussion(status(&a, "resolve_a", "resolved"))
        .unwrap();
    assert_eq!(resolved.status, DiscussionStatus::Resolved);
    assert!(store
        .write_discussion(reply(&resolved, "closed_reply"))
        .is_err());
    let reply_b = store.write_discussion(reply(&b, "reply_b")).unwrap();
    assert_eq!(reply_b.status, DiscussionStatus::Active);
    assert_eq!(
        store.list_threads(&scope()).unwrap()[0].status,
        ThreadStatus::Active
    );
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
    assert!(store.list_questions(&scope()).unwrap().is_empty());
    assert!(store.list_dispositions(&scope()).unwrap().is_empty());
    let reopened = store
        .write_discussion(status(&resolved, "reopen", "active"))
        .unwrap();
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
            store.write_discussion(status(&a, "resolve", "resolved"))
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
fn authorized_replay_survives_restart_and_refuses_changed_intent() {
    let (temp, store) = fixture();
    let input = write("root", json!({"kind":"start","role":"user","body":"A"}));
    let serialized = serde_json::to_value(&input).unwrap();
    let first = store.write_discussion(input).unwrap();
    store.write_discussion(reply(&first, "reply")).unwrap();
    let store = StateStore::new(ProvenanceLayout::new(
        camino::Utf8Path::from_path(temp.path()).unwrap(),
    ));
    assert_eq!(
        first,
        store
            .write_discussion(serde_json::from_value(serialized.clone()).unwrap())
            .unwrap()
    );
    let mut changed = serialized.clone();
    changed["action"]["body"] = json!("B");
    assert!(store
        .write_discussion(serde_json::from_value(changed).unwrap())
        .is_err());
    let mut owner = serialized;
    owner["declared_by"] = json!("other");
    assert!(store
        .write_discussion(serde_json::from_value(owner).unwrap())
        .is_err());
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
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
