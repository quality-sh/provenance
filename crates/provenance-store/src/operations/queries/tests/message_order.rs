//! The pages of the messages of a legacy thread whose message times are at
//! the two ends of the range of the read database, and the refusal of a
//! message time that would pass the range.
//!
//! No Rule: implementation aids. The first test holds the page order, the
//! cursor walk and the first page while the first page stops binding a
//! position. `discussion_outputs.txt` holds the bytes of the pages.

use super::{repository_at, root_of, seeded_store};
use crate::cache::tests::fixtures::append_record;
use crate::operations::read_policy::ReadPolicy;
use crate::review::read_discussion_messages;
use crate::state_store::StateStore;
use provenance_core::threads::{DiscussionMessagesQuery, DiscussionSelector};
use provenance_core::{StableId, ThreadParent};
use serde_json::json;

const LARGEST: i64 = (1 << 53) - 1;

fn parent() -> ThreadParent {
    serde_json::from_value(json!({"node_type": "requirement", "node_id": "req_overtime"})).unwrap()
}

/// A legacy thread on `req_overtime` with one message for each pair of an
/// id and a time.
fn legacy_messages(store: &StateStore, messages: &[(&str, i64)]) {
    let scope = provenance_core::ScopeId::new("default").unwrap();
    append_record(
        &crate::shards::threads_path(&store.layout, &scope),
        &json!({"scope_id": "default", "id": "thread_edge", "parent": parent(),
            "status": "active", "created_at": 1}),
    );
    let path = store
        .layout
        .state_dir()
        .join("scopes/default/threads/2026-09.jsonl");
    for (id, created_at) in messages {
        append_record(
            &path,
            &json!({"scope_id": "default", "id": id, "thread_id": "thread_edge",
                "role": "user", "body": format!("Message {id}"), "created_at": created_at}),
        );
    }
}

/// The message pages of a limit of 2 over messages at both ends of the
/// range, and at one time shared by two messages, are in time order and
/// then id order, and the cursors walk them to the end.
#[tokio::test]
async fn message_pages_walk_the_whole_range_in_order() {
    let (dir, store, _scope) = seeded_store();
    legacy_messages(
        &store,
        &[
            ("msg_high", LARGEST),
            ("msg_b", 0),
            ("msg_low", -LARGEST),
            ("msg_a", 0),
            ("msg_c", 7),
        ],
    );
    let root = root_of(&dir);
    let mut query = DiscussionMessagesQuery {
        parent: parent(),
        selector: DiscussionSelector::Legacy {
            thread_id: StableId::new("thread_edge").unwrap(),
        },
        limit: 2,
        cursor: None,
    };
    let mut pages = Vec::new();
    loop {
        let page =
            read_discussion_messages(&repository_at(&root), ReadPolicy::default(), query.clone())
                .await
                .unwrap()
                .result;
        pages.push(
            page.entries
                .iter()
                .map(|message| (message.id.as_str().to_owned(), message.created_at))
                .collect::<Vec<_>>(),
        );
        let Some(cursor) = page.next_cursor else {
            break;
        };
        query.cursor = Some(cursor);
    }
    let expected: Vec<Vec<(String, i64)>> = [
        vec![("msg_low", -LARGEST), ("msg_a", 0)],
        vec![("msg_b", 0), ("msg_c", 7)],
        vec![("msg_high", LARGEST)],
    ]
    .into_iter()
    .map(|page| {
        page.into_iter()
            .map(|(id, at)| (id.to_owned(), at))
            .collect()
    })
    .collect();
    assert_eq!(pages, expected);
}

/// A new message after a message at the top of the range would take a time
/// past the range, so the write refuses it and keeps nothing.
#[test]
fn a_message_time_past_the_range_is_refused() {
    let (_dir, store, scope) = seeded_store();
    legacy_messages(&store, &[("msg_high", LARGEST)]);
    let messages = store
        .layout
        .state_dir()
        .join("scopes/default/threads/2026-09.jsonl");
    let before = std::fs::read_to_string(&messages).unwrap();
    let mut input = json!({"scope_id": "default", "parent": parent(), "actor": "ben"});
    input["action"] = json!({"kind": "start", "role": "user", "body": "Concern"});
    let refused = store
        .write_discussion(serde_json::from_value(input).unwrap())
        .unwrap_err();
    assert!(
        format!("{refused:#}").contains("Message timestamp overflow"),
        "{refused:#}"
    );
    assert_eq!(std::fs::read_to_string(&messages).unwrap(), before);
    assert_eq!(store.list_messages(&scope).unwrap().len(), 1);
}
