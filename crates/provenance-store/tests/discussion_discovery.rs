mod discussion_support;

use discussion_support::*;
use provenance_core::{
    protocol::read_failure::ReadFailure,
    threads::{DiscussionConversationQuery, DiscussionListQuery, DiscussionStatusFilter},
    NodeType, StableId,
};
use provenance_store::{
    operations::read_policy::ReadPolicy,
    review::{read_discussion_conversation, read_discussion_list},
};

fn list(parent: Option<provenance_core::ThreadParent>, limit: usize) -> DiscussionListQuery {
    DiscussionListQuery {
        parent,
        allowed_parent_kinds: vec![NodeType::Requirement],
        status: DiscussionStatusFilter::Active,
        limit,
        cursor: None,
    }
}

#[tokio::test]
async fn selected_pages_are_complete_and_bind_status_parent_and_revision() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let a = start(&store, "a");
    let b = start(&store, "b");
    let c = start(&store, "c");
    store.write_discussion(status(&b, "resolve_b", "resolved")).unwrap();
    let mut query = list(None, 1);
    let mut found = Vec::new();
    loop {
        let page = read_discussion_list(root, &scope(), ReadPolicy::default(), query.clone())
            .await.unwrap().result;
        found.extend(page.entries.into_iter().map(|entry| entry.discussion_id));
        match page.next_cursor {
            Some(cursor) => query.cursor = Some(cursor),
            None => break,
        }
    }
    found.sort();
    let mut expected = vec![a.discussion_id.clone(), c.discussion_id.clone()];
    expected.sort();
    assert_eq!(found, expected);

    let first = read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 1))
        .await.unwrap().result;
    let cursor = first.next_cursor.unwrap();
    let mut wrong = list(None, 1);
    wrong.status = DiscussionStatusFilter::All;
    wrong.cursor = Some(cursor.clone());
    assert!(read_discussion_list(root, &scope(), ReadPolicy::default(), wrong).await
        .unwrap_err().downcast_ref::<ReadFailure>().is_some_and(|e| *e == ReadFailure::CursorInvalid));
    let mut wrong = list(Some(a.parent.clone()), 1);
    wrong.cursor = Some(cursor.clone());
    assert!(read_discussion_list(root, &scope(), ReadPolicy::default(), wrong).await
        .unwrap_err().downcast_ref::<ReadFailure>().is_some_and(|e| *e == ReadFailure::CursorInvalid));

    let mut resolved = list(None, 10);
    resolved.status = DiscussionStatusFilter::Resolved;
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), resolved).await.unwrap().result;
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].discussion_id, b.discussion_id);

    start(&store, "later");
    let mut stale = list(None, 1);
    stale.cursor = Some(cursor);
    assert!(read_discussion_list(root, &scope(), ReadPolicy::default(), stale).await
        .unwrap_err().downcast_ref::<ReadFailure>().is_some_and(|e| *e == ReadFailure::CursorRevisionChanged));
}

#[tokio::test]
async fn denied_kinds_and_missing_targets_have_no_rows_or_cursor() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let entry = start(&store, "visible");
    let mut query = list(None, 1);
    query.allowed_parent_kinds = vec![NodeType::Source];
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), query.clone())
        .await.unwrap().result;
    assert!(page.entries.is_empty());
    assert!(page.next_cursor.is_none());
    query.parent = Some(entry.parent.clone());
    assert!(read_discussion_list(root, &scope(), ReadPolicy::default(), query).await
        .unwrap_err().downcast_ref::<ReadFailure>().is_some_and(|e| *e == ReadFailure::ResourceNotFound));
    let denied = DiscussionConversationQuery {
        discussion_id: entry.discussion_id,
        allowed_parent_kinds: vec![NodeType::Source],
        limit: 1,
        cursor: None,
    };
    assert!(read_discussion_conversation(root, &scope(), ReadPolicy::default(), denied).await
        .unwrap_err().downcast_ref::<ReadFailure>().is_some_and(|e| *e == ReadFailure::ResourceNotFound));
    let missing = DiscussionConversationQuery {
        discussion_id: StableId::new("missing").unwrap(),
        allowed_parent_kinds: vec![NodeType::Requirement],
        limit: 1,
        cursor: None,
    };
    assert!(read_discussion_conversation(root, &scope(), ReadPolicy::default(), missing).await
        .unwrap_err().downcast_ref::<ReadFailure>().is_some_and(|e| *e == ReadFailure::ResourceNotFound));
}

#[tokio::test]
async fn conversation_returns_head_and_ordered_bounded_messages() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let entry = start(&store, "opening");
    let reply = store.write_discussion(reply(&entry, "answer")).unwrap();
    let mut query = DiscussionConversationQuery {
        discussion_id: entry.discussion_id,
        allowed_parent_kinds: vec![NodeType::Requirement],
        limit: 1,
        cursor: None,
    };
    let first = read_discussion_conversation(root, &scope(), ReadPolicy::default(), query.clone())
        .await.unwrap().result;
    assert_eq!(first.head.version, reply.version);
    assert_eq!(first.messages.entries[0].body, "opening");
    query.cursor = first.messages.next_cursor;
    let second = read_discussion_conversation(root, &scope(), ReadPolicy::default(), query)
        .await.unwrap().result;
    assert_eq!(second.head.version, reply.version);
    assert_eq!(second.messages.entries[0].body, "answer");
    assert!(second.messages.next_cursor.is_none());
}
