//! Discussion message continuations bind the effective page limit.
mod discussion_support;
use discussion_support::*;
use provenance_core::{
    protocol::read_failure::ReadFailure,
    threads::{DiscussionEntry, DiscussionMessagesQuery, DiscussionSelector},
};
use provenance_macros::verifies;
use provenance_store::{operations::read_policy::ReadPolicy, review::read_discussion_messages};
use serde_json::json;

fn messages_query(latest: &DiscussionEntry, limit: usize) -> DiscussionMessagesQuery {
    DiscussionMessagesQuery {
        parent: latest.parent.clone(),
        selector: DiscussionSelector::Discussion {
            discussion_id: latest.discussion_id.clone(),
        },
        limit,
        cursor: None,
    }
}

#[tokio::test]
#[verifies("rule_cursor_binds_query_identity", examples)]
async fn message_pages_continue_at_the_same_limit_and_refuse_a_changed_limit() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let mut latest = start(&store, "a");
    for n in 0..4 {
        latest = store
            .write_discussion(reply(&latest, &format!("reply_{n}")))
            .unwrap();
    }
    let query = messages_query(&latest, 2);
    let first = read_discussion_messages(root, &scope(), ReadPolicy::default(), query.clone())
        .await
        .unwrap();
    assert_eq!(first.result.entries.len(), 2);
    let cursor = first
        .result
        .next_cursor
        .clone()
        .expect("page one continues");
    let second = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionMessagesQuery {
            cursor: Some(cursor.clone()),
            ..query.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(second.result.entries.len(), 2);
    assert!(second.result.next_cursor.is_some(), "page two continues");
    let changed = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionMessagesQuery {
            cursor: Some(cursor),
            limit: 3,
            ..query
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        changed.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::CursorInvalid)
    );
}

#[tokio::test]
#[verifies("rule_cursor_binds_query_identity", examples)]
async fn an_omitted_limit_continues_a_default_limit_page() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let mut latest = start(&store, "a");
    for n in 0..50 {
        latest = store
            .write_discussion(reply(&latest, &format!("reply_{n}")))
            .unwrap();
    }
    let parent = serde_json::to_value(&latest.parent).unwrap();
    let selector = json!({
        "kind":"discussion",
        "discussion_id": latest.discussion_id.as_str()
    });
    let first = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        serde_json::from_value(json!({
            "parent": parent,
            "selector": selector,
            "limit": 50,
            "cursor": null
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(first.result.entries.len(), 50);
    let cursor = first.result.next_cursor.expect("page one continues");
    let second = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        serde_json::from_value(json!({
            "parent": parent,
            "selector": selector,
            "cursor": cursor
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(second.result.entries.len(), 1);
    assert!(second.result.next_cursor.is_none());
}
