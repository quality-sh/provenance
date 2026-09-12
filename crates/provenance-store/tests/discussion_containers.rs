mod discussion_support;
use discussion_support::*;
use provenance_core::{Thread, ThreadStatus};
use provenance_store::{layout::ProvenanceLayout, shards};
use serde_json::json;

fn replace_threads(
    store: &provenance_store::state_store::StateStore,
    temp: &tempfile::TempDir,
    change: impl FnOnce(&mut Vec<Thread>),
) {
    let mut threads = store.list_threads(&scope()).unwrap();
    change(&mut threads);
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let mut bytes = Vec::new();
    for thread in &threads {
        serde_json::to_writer(&mut bytes, thread).unwrap();
        bytes.push(b'\n');
    }
    std::fs::write(shards::threads_path(&layout, &scope()), bytes).unwrap();
}

#[test]
fn closed_containers_refuse_reply_and_reopening_without_reactivation() {
    for closed in [ThreadStatus::Resolved, ThreadStatus::Archived] {
        let (temp, store) = fixture();
        let a = start(&store, "a");
        let resolved = store
            .write_discussion(status(&a, "resolve", "resolved"))
            .unwrap();
        replace_threads(&store, &temp, |threads| threads[0].status = closed.clone());
        assert!(store.write_discussion(reply(&a, "reply")).is_err());
        assert!(store
            .write_discussion(status(&resolved, "reopen", "active"))
            .is_err());
        let b = start(&store, "b");
        assert_ne!(a.thread_id, b.thread_id);
        let history = store.list_threads(&scope()).unwrap();
        assert_eq!(
            history.iter().find(|t| t.id == a.thread_id).unwrap().status,
            closed
        );
        assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
    }
}

#[test]
fn reply_reconciles_siblings_but_cannot_move_a_losing_discussion() {
    let (temp, store) = fixture();
    let a = start(&store, "a");
    replace_threads(&store, &temp, |threads| {
        let mut sibling = threads[0].clone();
        sibling.schema_version = provenance_core::SUPPORTED_SCHEMA_VERSION;
        sibling.id = provenance_core::StableId::new("later").unwrap();
        sibling.created_at += 1;
        threads.push(sibling);
    });
    let replied = store.write_discussion(reply(&a, "reply")).unwrap();
    assert_eq!(
        store
            .list_threads(&scope())
            .unwrap()
            .iter()
            .find(|t| t.id.as_str() == "later")
            .unwrap()
            .status,
        ThreadStatus::Archived
    );
    replace_threads(&store, &temp, |threads| {
        let mut older = threads[0].clone();
        older.schema_version = provenance_core::SUPPORTED_SCHEMA_VERSION;
        older.id = provenance_core::StableId::new("older").unwrap();
        older.status = ThreadStatus::Active;
        older.created_at = -1;
        threads.push(older);
    });
    assert!(store
        .write_discussion(reply(&replied, "losing_reply"))
        .is_err());
    let b = start(&store, "new_root");
    assert_eq!(b.thread_id.as_str(), "older");
    let messages = store.list_messages(&scope()).unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|m| m.thread_id == a.thread_id)
            .count(),
        2
    );
    assert!(store
        .write_discussion(reply(&replied, "archived_reply"))
        .is_err());
}

#[test]
fn old_post_and_direct_enrolled_mutation_refuse_before_publication() {
    let (temp, store) = fixture();
    let a = start(&store, "a");
    assert!(store
        .post_thread_message(
            serde_json::from_value(
                json!({"scope_id":"default","parent":a.parent,"role":"user","body":"bypass"})
            )
            .unwrap()
        )
        .is_err());
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    assert!(provenance_store::jsonl::write_jsonl_atomic(
        &shards::threads_path(&layout, &scope()),
        &Vec::<Thread>::new()
    )
    .is_err());
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[test]
fn direct_message_append_cannot_bypass_enrolled_membership() {
    let (temp, store) = fixture();
    let a = start(&store, "a");
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let mut messages = store.list_messages(&scope()).unwrap();
    let mut bypass = messages[0].clone();
    bypass.id = provenance_core::StableId::new("bypass").unwrap();
    bypass.schema_version = provenance_core::SUPPORTED_SCHEMA_VERSION;
    messages.push(bypass);
    assert!(provenance_store::jsonl::write_jsonl_atomic(
        &shards::messages_path(&layout, &scope()),
        &messages
    )
    .is_err());
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
    assert_eq!(
        store.list_messages(&scope()).unwrap()[0].id,
        a.message_id.unwrap()
    );
}

#[test]
fn resolving_a_discussion_leaves_all_container_statuses_unchanged() {
    let (temp, store) = fixture();
    let a = start(&store, "a");
    replace_threads(&store, &temp, |threads| {
        let mut later = threads[0].clone();
        later.schema_version = provenance_core::SUPPORTED_SCHEMA_VERSION;
        later.id = provenance_core::StableId::new("later").unwrap();
        later.created_at += 1;
        threads.push(later);
    });
    let before = store.list_threads(&scope()).unwrap();
    store
        .write_discussion(status(&a, "resolve", "resolved"))
        .unwrap();
    assert_eq!(before, store.list_threads(&scope()).unwrap());
}

#[test]
fn legacy_messages_in_an_enrolled_thread_cannot_be_deleted_by_an_old_writer() {
    let (temp, store) = fixture();
    let old=store.post_thread_message(serde_json::from_value(json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_a"},"role":"user","body":"Legacy"})).unwrap()).unwrap();
    start(&store, "root");
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let messages = store
        .list_messages(&scope())
        .unwrap()
        .into_iter()
        .filter(|m| m.id != old.message.id)
        .collect::<Vec<_>>();
    assert!(provenance_store::jsonl::write_jsonl_atomic(
        &shards::messages_path(&layout, &scope()),
        &messages
    )
    .is_err());
}

#[test]
fn posting_refuses_a_container_stored_under_the_wrong_scope() {
    let (temp, store) = fixture();
    let old=store.post_thread_message(serde_json::from_value(json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_a"},"role":"user","body":"Legacy"})).unwrap()).unwrap();
    replace_threads(&store, &temp, |threads| {
        threads[0].scope_id = provenance_core::ScopeId::new("elsewhere").unwrap();
    });
    assert!(store
        .write_discussion(write(
            "bad",
            json!({"kind":"start","role":"user","body":"Bad scope"})
        ))
        .is_err());
    assert_eq!(store.list_messages(&scope()).unwrap()[0].id, old.message.id);
}

#[test]
fn new_messages_do_not_reuse_identities_from_legacy_shards() {
    let (temp, store) = fixture();
    let old=store.post_thread_message(serde_json::from_value(json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_a"},"role":"user","body":"Legacy"})).unwrap()).unwrap();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let path = shards::messages_path(&layout, &scope());
    std::fs::rename(&path, path.parent().unwrap().join("2020-01.jsonl")).unwrap();
    let a = start(&store, "new");
    assert_ne!(a.message_id.unwrap(), old.message.id);
}
