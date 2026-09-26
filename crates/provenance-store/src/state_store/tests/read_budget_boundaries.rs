//! Boundaries shared by public resource pages and discussion documents.

use super::initialized_store;
use super::read_budget::{blank_source, list_read, member_read, source_of_stored_bytes};
use crate::state_store::{read_budget::ReadBudget, PostMessageInput};
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{
    MessageRole, NodeType, ScopeId, StableId, Thread, ThreadParent, ThreadStatus,
    SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;

const RESOURCE_BYTES: usize = crate::cache::read::page::RESOURCE_RECORD_BYTES;
const DOCUMENT_BYTES: usize = crate::cache::read::page::RECORD_BYTES;

fn json_boundary_sql_bytes(scope: &ScopeId, id: &str) -> usize {
    let blank = blank_source(scope, id);
    let json = serde_json::to_vec(&blank).unwrap().len();
    let sql = blank.read_bytes().unwrap();
    assert!(json > sql);
    RESOURCE_BYTES - (json - sql)
}

fn assert_too_large<T>(result: anyhow::Result<T>) {
    let Err(error) = result else {
        panic!("write must refuse the oversized record");
    };
    let failure = error
        .downcast_ref::<SourceFailure>()
        .expect("typed refusal");
    assert!(matches!(failure.failure, WriteFailure::RecordTooLarge));
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn source_at_json_page_boundary_is_accepted_and_readable() {
    let (dir, store, scope) = initialized_store();
    let id = "source_json_edge";
    let source = store
        .create_source(source_of_stored_bytes(
            &scope,
            id,
            json_boundary_sql_bytes(&scope, id),
        ))
        .expect("exact JSON page boundary is accepted");
    assert_eq!(serde_json::to_vec(&source).unwrap().len(), RESOURCE_BYTES);
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    assert_eq!(member_read(&root, id).await.unwrap()["result"]["id"], id);
    assert_eq!(
        list_read(&root).await.unwrap()["result"]["items"][0]["id"],
        id
    );
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn source_above_json_page_boundary_is_refused_before_publication() {
    let (dir, store, scope) = initialized_store();
    let id = "source_json_over";
    let target = json_boundary_sql_bytes(&scope, id) + 1;
    assert!(
        target <= RESOURCE_BYTES,
        "SQLite budget still accepts this row"
    );
    assert_too_large(store.create_source(source_of_stored_bytes(&scope, id, target)));
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    assert_eq!(
        member_read(&root, id).await.unwrap_err(),
        "resource_not_found"
    );
    assert!(list_read(&root).await.unwrap()["result"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn long_source_id_is_refused_before_resource_page_scan() {
    let (dir, store, scope) = initialized_store();
    let id = "x".repeat(1025);
    assert_too_large(store.create_source(source_of_stored_bytes(&scope, &id, 4096)));
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    assert!(list_read(&root).await.unwrap()["result"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
}

fn parent(id: String) -> ThreadParent {
    ThreadParent {
        node_type: NodeType::Requirement,
        node_id: StableId::new(id).unwrap(),
    }
}

fn post(scope: &ScopeId, parent: &ThreadParent) -> PostMessageInput {
    PostMessageInput {
        scope_id: scope.clone(),
        parent: parent.clone(),
        role: MessageRole::User,
        body: "small".into(),
    }
}

fn thread(scope: &ScopeId, parent: &ThreadParent, id: &str, created_at: i64) -> Thread {
    Thread {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        parent: parent.clone(),
        status: ThreadStatus::Active,
        created_at,
    }
}

fn document_bytes(thread: &Thread) -> usize {
    thread.scope_id.as_str().len()
        + thread.id.as_str().len()
        + serde_json::to_string(&thread.parent.node_type)
            .unwrap()
            .len()
        - 2
        + thread.parent.node_id.as_str().len()
        + serde_json::to_string(&thread.status).unwrap().len()
        - 2
        + thread.created_at.to_string().len()
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn long_parent_refuses_new_thread_before_thread_or_message_publication() {
    let (_dir, store, scope) = initialized_store();
    let parent = parent("x".repeat(40_000));
    assert_too_large(store.post_thread_message(post(&scope, &parent)));
    assert!(store.list_threads(&scope).unwrap().is_empty());
    assert!(store.list_messages(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn generated_thread_id_above_document_key_limit_is_refused() {
    let (_dir, store, scope) = initialized_store();
    let parent = parent("x".repeat(1010));
    assert_too_large(store.post_thread_message(post(&scope, &parent)));
    assert!(store.list_threads(&scope).unwrap().is_empty());
    assert!(store.list_messages(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn long_parent_refuses_existing_thread_before_message_publication() {
    let (_dir, store, scope) = initialized_store();
    let parent = parent("x".repeat(DOCUMENT_BYTES));
    let existing = thread(&scope, &parent, "thread_existing", 1);
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::threads_path(&store.layout, &scope),
        &[existing],
    )
    .unwrap();
    let path = crate::shards::threads_path(&store.layout, &scope);
    let before = std::fs::read(&path).unwrap();
    assert_too_large(store.post_thread_message(post(&scope, &parent)));
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(store.list_messages(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn archiving_sibling_over_document_boundary_refuses_whole_post() {
    let (_dir, store, scope) = initialized_store();
    let short_parent = parent("x".into());
    let base = document_bytes(&thread(&scope, &short_parent, "thread_a", 1)) - 1;
    let parent = parent("x".repeat(DOCUMENT_BYTES - base - 1));
    let canonical = thread(&scope, &parent, "thread_a", 1);
    let sibling = thread(&scope, &parent, "thread_b", 2);
    assert_eq!(document_bytes(&sibling), DOCUMENT_BYTES - 1);
    let path = crate::shards::threads_path(&store.layout, &scope);
    crate::jsonl::write_jsonl_atomic(&path, &[canonical, sibling]).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert_too_large(store.post_thread_message(post(&scope, &parent)));
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(store.list_messages(&scope).unwrap().is_empty());
}
