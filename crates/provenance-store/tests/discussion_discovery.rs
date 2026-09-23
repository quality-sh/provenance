mod discussion_support;

use discussion_support::*;
use provenance_core::{
    protocol::read_failure::ReadFailure,
    threads::{DiscussionConversationQuery, DiscussionListQuery, DiscussionStatusFilter},
    NodeType, StableId,
};
use provenance_macros::verifies;
use provenance_store::{
    operations::{
        catalog::{ListDiscussionsV2, Operation, PreparedContext, PreparedRead},
        read_policy::ReadPolicy,
    },
    review::{read_discussion_conversation, read_discussion_list},
};
use serde_json::json;

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
#[verifies("rule_porcelain_discussions_list_scope_or_parent", examples)]
#[verifies("rule_porcelain_discussions_select_status", examples)]
async fn selected_pages_are_complete_and_bind_status_parent_and_revision() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let a = start(&store, "a");
    let b = start(&store, "b");
    let c = start(&store, "c");
    store
        .write_discussion(status(&b, "resolve_b", "resolved"))
        .unwrap();
    let mut query = list(None, 1);
    let mut found = Vec::new();
    loop {
        let page = read_discussion_list(root, &scope(), ReadPolicy::default(), query.clone())
            .await
            .unwrap()
            .result;
        found.extend(page.entries.into_iter().map(|entry| entry.discussion_id));
        match page.next_cursor {
            Some(cursor) => query.cursor = Some(cursor),
            None => break,
        }
    }
    found.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let mut expected = vec![a.discussion_id.clone(), c.discussion_id.clone()];
    expected.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(found, expected);

    let first = read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 1))
        .await
        .unwrap()
        .result;
    let cursor = first.next_cursor.unwrap();
    let mut wrong = list(None, 1);
    wrong.status = DiscussionStatusFilter::All;
    wrong.cursor = Some(cursor.clone());
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), wrong)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::CursorInvalid)
    );
    let mut wrong = list(Some(a.parent.clone()), 1);
    wrong.cursor = Some(cursor.clone());
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), wrong)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::CursorInvalid)
    );

    let mut resolved = list(None, 10);
    resolved.status = DiscussionStatusFilter::Resolved;
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), resolved)
        .await
        .unwrap()
        .result;
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].discussion_id, b.discussion_id);
    let mut all = list(None, 10);
    all.status = DiscussionStatusFilter::All;
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), all)
        .await
        .unwrap()
        .result;
    assert_eq!(page.entries.len(), 3);
    assert!(page
        .entries
        .iter()
        .any(|entry| entry.discussion_id == b.discussion_id));

    start(&store, "later");
    let mut stale = list(None, 1);
    stale.cursor = Some(cursor);
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), stale)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::CursorRevisionChanged)
    );
}

#[tokio::test]
async fn denied_kinds_and_missing_targets_have_no_rows_or_cursor() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let entry = start(&store, "visible");
    let mut query = list(None, 1);
    query.allowed_parent_kinds = vec![NodeType::Source];
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), query.clone())
        .await
        .unwrap()
        .result;
    assert!(page.entries.is_empty());
    assert!(page.next_cursor.is_none());
    query.parent = Some(entry.parent.clone());
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), query)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::ResourceNotFound)
    );
    let denied = DiscussionConversationQuery {
        discussion_id: entry.discussion_id.clone(),
        allowed_parent_kinds: vec![NodeType::Source],
        limit: 1,
        cursor: None,
    };
    assert!(
        read_discussion_conversation(root, &scope(), ReadPolicy::default(), denied)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::ResourceNotFound)
    );
    let missing = DiscussionConversationQuery {
        discussion_id: StableId::new("missing").unwrap(),
        allowed_parent_kinds: vec![NodeType::Requirement],
        limit: 1,
        cursor: None,
    };
    assert!(
        read_discussion_conversation(root, &scope(), ReadPolicy::default(), missing)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::ResourceNotFound)
    );
}

#[tokio::test]
async fn conversation_returns_head_and_ordered_bounded_messages() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let entry = start(&store, "opening");
    let reply = store.write_discussion(reply(&entry, "answer")).unwrap();
    let mut query = DiscussionConversationQuery {
        discussion_id: entry.discussion_id.clone(),
        allowed_parent_kinds: vec![NodeType::Requirement],
        limit: 1,
        cursor: None,
    };
    let first = read_discussion_conversation(root, &scope(), ReadPolicy::default(), query.clone())
        .await
        .unwrap()
        .result;
    assert_eq!(first.head.version, reply.version);
    assert_eq!(first.messages.entries[0].body, "opening");
    query.cursor = first.messages.next_cursor.clone();
    let second = read_discussion_conversation(root, &scope(), ReadPolicy::default(), query)
        .await
        .unwrap()
        .result;
    assert_eq!(second.head.version, reply.version);
    assert_eq!(second.messages.entries[0].body, "answer");
    assert!(second.messages.next_cursor.is_none());

    let other = start(&store, "other");
    let mut wrong = DiscussionConversationQuery {
        discussion_id: other.discussion_id,
        allowed_parent_kinds: vec![NodeType::Requirement],
        limit: 1,
        cursor: first.messages.next_cursor.clone(),
    };
    assert!(
        read_discussion_conversation(root, &scope(), ReadPolicy::default(), wrong.clone())
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::CursorInvalid)
    );
    wrong.discussion_id = entry.discussion_id;
    assert!(
        read_discussion_conversation(root, &scope(), ReadPolicy::default(), wrong)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::CursorRevisionChanged)
    );
}

#[tokio::test]
#[verifies("rule_porcelain_discussions_default_active", examples)]
#[verifies("rule_porcelain_discussion_list_entry", examples)]
async fn default_list_has_truthful_parent_status_and_bounded_opening() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let empty = read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 10))
        .await
        .unwrap()
        .result;
    assert!(empty.entries.is_empty());
    assert!(empty.next_cursor.is_none());
    store
        .post_thread_message(
            serde_json::from_value(json!({
                "scope_id":"default", "parent":{"node_type":"requirement","node_id":"req_a"},
                "role":"user", "body":"legacy message"
            }))
            .unwrap(),
        )
        .unwrap();
    let legacy_only = read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 10))
        .await
        .unwrap()
        .result;
    assert!(legacy_only.entries.is_empty());
    assert!(legacy_only.next_cursor.is_none());
    let body = "α".repeat(300);
    let entry = store
        .write_discussion(write(
            "long",
            json!({
                "kind":"start", "role":"user", "body":body
            }),
        ))
        .unwrap();
    let query: DiscussionListQuery = serde_json::from_value(json!({
        "parent": entry.parent, "allowed_parent_kinds":["requirement"],
        "limit":10, "cursor":null
    }))
    .unwrap();
    assert_eq!(query.status, DiscussionStatusFilter::Active);
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), query)
        .await
        .unwrap()
        .result;
    assert_eq!(page.entries.len(), 1);
    let summary = &page.entries[0];
    assert_eq!(summary.discussion_id, entry.discussion_id);
    assert_eq!(summary.parent, entry.parent);
    assert_eq!(summary.status, entry.status);
    assert_eq!(summary.opening_excerpt.chars().count(), 240);
    assert!(summary.excerpt_truncated);
}

#[tokio::test]
async fn page_limits_and_large_messages_are_bounded() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let entry = store
        .write_discussion(write(
            "large",
            json!({
                "kind":"start", "role":"user", "body":"x".repeat(60_000)
            }),
        ))
        .unwrap();
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 10))
        .await
        .unwrap()
        .result;
    assert_eq!(page.entries.len(), 1);
    assert!(page.entries[0].excerpt_truncated);
    let conversation = DiscussionConversationQuery {
        discussion_id: entry.discussion_id,
        allowed_parent_kinds: vec![NodeType::Requirement],
        limit: 10,
        cursor: None,
    };
    let read = read_discussion_conversation(root, &scope(), ReadPolicy::default(), conversation)
        .await
        .unwrap()
        .result;
    assert_eq!(read.messages.entries.len(), 1);
    assert_eq!(read.messages.entries[0].body.len(), 60_000);
    assert!(read.messages.next_cursor.is_none());
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 0))
            .await
            .is_err()
    );
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 201))
            .await
            .is_err()
    );
    let mut oversized_cursor = list(None, 1);
    oversized_cursor.cursor = Some("x".repeat(8193));
    assert!(
        read_discussion_list(root, &scope(), ReadPolicy::default(), oversized_cursor)
            .await
            .unwrap_err()
            .downcast_ref::<ReadFailure>()
            .is_some_and(|e| *e == ReadFailure::CursorInvalid)
    );
}

#[tokio::test]
async fn catalog_operation_applies_grants_before_paging() {
    let (temp, store) = fixture();
    start(&store, "one");
    start(&store, "two");
    let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let context = PreparedContext::read(PreparedRead {
        root,
        scope: scope(),
        policy: ReadPolicy::default(),
        requested_target: "fixture".into(),
        external: false,
    });
    let mut query = list(None, 1);
    query.allowed_parent_kinds = vec![NodeType::Source];
    let denied = ListDiscussionsV2::run(context.clone(), query)
        .await
        .unwrap();
    assert!(denied.result.entries.is_empty());
    assert!(!denied.result.has_more);
    assert!(denied.result.next_cursor.is_none());
    let allowed = ListDiscussionsV2::run(context, list(None, 1))
        .await
        .unwrap();
    assert_eq!(allowed.result.entries.len(), 1);
    assert!(allowed.result.has_more);
}

#[tokio::test]
async fn scope_list_includes_other_parents_and_parent_list_does_not() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let a = start(&store, "first");
    store.create_review_requirement(serde_json::from_value(json!({
        "request_id":"create_b", "actor":"ben", "origin":null,
        "create": {
            "scope_id":"default", "id":"req_b", "statement":"The system stores another record.",
            "status":"discovery", "depends_on":[], "supersedes":[]
        }
    })).unwrap()).unwrap();
    let mut input = write(
        "second",
        json!({"kind":"start","role":"user","body":"second"}),
    );
    input.parent.node_id = StableId::new("req_b").unwrap();
    let b = store.write_discussion(input).unwrap();
    let scope_page = read_discussion_list(root, &scope(), ReadPolicy::default(), list(None, 10))
        .await
        .unwrap()
        .result;
    assert_eq!(scope_page.entries.len(), 2);
    assert!(scope_page
        .entries
        .iter()
        .any(|entry| entry.discussion_id == a.discussion_id));
    assert!(scope_page
        .entries
        .iter()
        .any(|entry| entry.discussion_id == b.discussion_id));
    let parent_page = read_discussion_list(
        root,
        &scope(),
        ReadPolicy::default(),
        list(Some(a.parent), 10),
    )
    .await
    .unwrap()
    .result;
    assert_eq!(parent_page.entries.len(), 1);
    assert_eq!(parent_page.entries[0].discussion_id, a.discussion_id);
}

#[tokio::test]
async fn concurrent_replies_do_not_split_head_from_messages() {
    let (temp, store) = fixture();
    let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let first = start(&store, "root");
    let discussion_id = first.discussion_id.clone();
    let writer = std::thread::spawn(move || {
        let mut head = first;
        for index in 0..12 {
            head = store
                .write_discussion(reply(&head, &format!("reply_{index}")))
                .unwrap();
        }
    });
    for _ in 0..20 {
        let result = read_discussion_conversation(
            &root,
            &scope(),
            ReadPolicy::default(),
            DiscussionConversationQuery {
                discussion_id: discussion_id.clone(),
                allowed_parent_kinds: vec![NodeType::Requirement],
                limit: 200,
                cursor: None,
            },
        )
        .await
        .unwrap()
        .result;
        assert_eq!(result.messages.entries.len(), result.head.version as usize);
        assert!(result.messages.next_cursor.is_none());
        tokio::task::yield_now().await;
    }
    writer.join().unwrap();
}
