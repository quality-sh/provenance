//! Writes in a stage of a memory repository read the state of that stage.
//!
//! A memory repository has no state files. A use case that reads the files
//! of the working tree in place of the stage finds no records there. These
//! tests show that scope import and Discussion removal see the records that
//! the state holds.
//!
//! The tests run with and without the `host` feature.

#[path = "support/memory.rs"]
mod memory;

use memory::{id, imported, key, memory_repository, memory_repository_with, scope, shard};
use provenance_core::{
    threads::DiscussionStatus, Message, MessageRole, NodeType, Source, Thread, ThreadParent,
};
use provenance_macros::verifies;
use provenance_store::review::{DiscussionAction, WriteDiscussion};
use provenance_store::state_store::{ScopeShards, TypedSpecInput};
use provenance_store::storage::StateStorage;
use serde_json::json;

fn source_row() -> serde_json::Value {
    json!({
        "scope_id": "imported", "id": "source_policy", "name": "Sharing policy",
        "source_type": "policy", "supersedes": [],
    })
}

/// A Thread and a Message with reserved ids, which a new record cannot use.
/// An earlier build wrote them.
fn reserved_rows() -> (serde_json::Value, serde_json::Value) {
    let thread = json!({
        "scope_id": "imported", "id": "answer",
        "parent": {"node_type": "source", "node_id": "source_policy"},
        "status": "active", "created_at": 1,
    });
    let message = json!({
        "scope_id": "imported", "id": "api", "thread_id": "answer",
        "role": "user", "body": "An earlier note.", "created_at": 1,
    });
    (thread, message)
}

fn record<T: serde::de::DeserializeOwned>(row: &serde_json::Value) -> T {
    serde_json::from_value(row.clone()).unwrap()
}

#[test]
#[verifies("rule_engine_runs_on_any_storage", examples)]
fn a_scope_import_on_memory_keeps_the_reserved_ids_that_the_state_holds() {
    let (thread, message) = reserved_rows();
    let memory = memory_repository_with([
        (
            key("scopes/imported/sources/source.jsonl"),
            shard(&[source_row()]),
        ),
        (
            key("scopes/imported/threads/threads.jsonl"),
            shard(std::slice::from_ref(&thread)),
        ),
        (
            key("scopes/imported/threads/2026-07.jsonl"),
            shard(std::slice::from_ref(&message)),
        ),
    ]);
    let sources: Vec<Source> = vec![record(&source_row())];
    let threads: Vec<Thread> = vec![record(&thread)];
    let messages: Vec<Message> = vec![record(&message)];

    memory
        .repository
        .stage(false, None, |staged| {
            staged.import_scope(
                &imported(),
                &ScopeShards {
                    sources: &sources,
                    threads: &threads,
                    messages: &messages,
                    ..ScopeShards::default()
                },
            )
        })
        .unwrap();

    let view = memory.repository.view().unwrap();
    assert_eq!(view.list_threads(&imported()).unwrap(), threads);
    assert_eq!(view.list_messages(&imported()).unwrap(), messages);

    let mut new_thread = threads[0].clone();
    new_thread.id = id("assertions");
    let before = memory.storage.read_view().unwrap();
    let refused = memory
        .repository
        .stage(false, None, |staged| {
            staged.import_scope(
                &imported(),
                &ScopeShards {
                    sources: &sources,
                    threads: std::slice::from_ref(&new_thread),
                    ..ScopeShards::default()
                },
            )
        })
        .unwrap_err();
    assert!(
        format!("{refused:#}").contains("reserved record ID assertions cannot be assigned"),
        "a new reserved id is refused: {refused:#}"
    );
    assert_eq!(memory.storage.read_view().unwrap(), before);
    memory.assert_empty();
}

fn typed_spec(sources: &serde_json::Value) -> TypedSpecInput {
    serde_json::from_value(json!({
        "spec": "fx", "declared_by": "spec://fx", "sources": sources, "requirements": [],
    }))
    .unwrap()
}

#[test]
#[verifies("rule_engine_runs_on_any_storage", examples)]
fn a_typed_spec_apply_on_memory_deletes_the_messages_of_a_removed_discussion() {
    let memory = memory_repository();
    let store = memory.repository.store();
    let legacy =
        json!([{"key": "legacy", "id": "source_legacy", "name": "Legacy", "kind": "policy"}]);
    store
        .apply_typed_spec(&scope(), typed_spec(&legacy))
        .unwrap();
    let started = store
        .write_discussion(WriteDiscussion {
            scope_id: scope(),
            parent: ThreadParent {
                node_type: NodeType::Source,
                node_id: id("source_legacy"),
            },
            actor: "reviewer".into(),
            declared_by: Some("spec://fx".into()),
            action: DiscussionAction::Start {
                role: MessageRole::User,
                body: "A concern.".into(),
            },
        })
        .unwrap();
    assert_eq!(started.status, DiscussionStatus::Active);
    let view = memory.repository.view().unwrap();
    assert_eq!(view.list_messages(&scope()).unwrap().len(), 1);

    let applied = store
        .apply_typed_spec(&scope(), typed_spec(&json!([])))
        .unwrap();

    assert_eq!(applied.deleted, 1);
    let view = memory.repository.view().unwrap();
    assert_eq!(view.list_discussions(&scope()).unwrap(), Vec::new());
    assert_eq!(view.list_threads(&scope()).unwrap(), Vec::new());
    assert!(
        view.list_messages(&scope()).unwrap().is_empty(),
        "the Messages of the removed Discussion stay"
    );
    memory.assert_empty();
}
