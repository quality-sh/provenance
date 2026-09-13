mod discussion_support;
use discussion_support::*;
use provenance_core::{
    threads::{DiscussionGroup, DiscussionMessagesQuery, DiscussionQuery, DiscussionSelector},
    StableId,
};
use provenance_store::{
    operations::read_policy::ReadPolicy,
    review::{read_discussion_messages, read_discussions},
};
use serde_json::json;

#[tokio::test]
async fn legacy_membership_is_explicit_and_pages_rebuild_from_journal() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let old=store.post_thread_message(serde_json::from_value(json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_a"},"role":"user","body":"Unknown root"})).unwrap()).unwrap();
    let a = start(&store, "a");
    store.write_discussion(reply(&a, "reply")).unwrap();
    let query = DiscussionQuery {
        parent: a.parent.clone(),
        limit: 1,
        cursor: None,
    };
    let page = read_discussions(root, &scope(), ReadPolicy::default(), query.clone())
        .await
        .unwrap();
    assert!(
        matches!(&page.result.entries[0],DiscussionGroup::Addressed {discussion,..} if discussion.version==2)
    );
    let next = read_discussions(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionQuery {
            cursor: page.result.next_cursor.clone(),
            ..query.clone()
        },
    )
    .await
    .unwrap();
    assert!(
        matches!(&next.result.entries[0],DiscussionGroup::Legacy {thread_id,..} if *thread_id==old.thread.id)
    );
    let messages = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionMessagesQuery {
            parent: a.parent.clone(),
            selector: DiscussionSelector::Discussion {
                discussion_id: a.discussion_id.clone(),
            },
            limit: 50,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(messages.result.entries.len(), 2);
    assert!(!messages
        .result
        .entries
        .iter()
        .any(|m| m.id == old.message.id));
    let legacy = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionMessagesQuery {
            parent: a.parent.clone(),
            selector: DiscussionSelector::Legacy {
                thread_id: old.thread.id,
            },
            limit: 50,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(legacy.result.entries.len(), 1);
    assert_eq!(legacy.result.entries[0].id, old.message.id);
    let b = start(&store, "b");
    assert!(read_discussions(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionQuery {
            cursor: page.result.next_cursor,
            ..query.clone()
        }
    )
    .await
    .is_err());
    let mut wrong = query.clone();
    wrong.parent.node_id = StableId::new("other").unwrap();
    assert!(
        read_discussions(root, &scope(), ReadPolicy::default(), wrong)
            .await
            .is_err()
    );
    let rebuilt = read_discussions(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionQuery {
            limit: 200,
            ..query
        },
    )
    .await
    .unwrap();
    assert_eq!(rebuilt.result.entries.len(), 3);
    assert!(rebuilt.result.entries.iter().any(|g|matches!(g,DiscussionGroup::Addressed {discussion,..} if discussion.discussion_id==b.discussion_id)));
}

#[tokio::test]
async fn message_selector_refuses_wrong_parent_and_missing_discussion() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let a = start(&store, "a");
    store.create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_other","statement":"The system retains records.","status":"discovery","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    for (parent, id) in [
        (a.parent.clone(), StableId::new("missing").unwrap()),
        (
            provenance_core::ThreadParent {
                node_type: provenance_core::NodeType::Requirement,
                node_id: StableId::new("req_other").unwrap(),
            },
            a.discussion_id,
        ),
    ] {
        assert!(read_discussion_messages(
            root,
            &scope(),
            ReadPolicy::default(),
            DiscussionMessagesQuery {
                parent,
                selector: DiscussionSelector::Discussion { discussion_id: id },
                limit: 50,
                cursor: None
            }
        )
        .await
        .is_err());
    }
}

#[tokio::test]
async fn missing_membership_is_a_conflict_instead_of_a_legacy_group() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let a = start(&store, "a");
    let dir = provenance_store::layout::ProvenanceLayout::new(root)
        .scopes_dir()
        .join("default/review/journal");
    for file in std::fs::read_dir(dir).unwrap() {
        std::fs::remove_file(file.unwrap().path()).unwrap();
    }
    assert!(read_discussions(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionQuery {
            parent: a.parent,
            limit: 50,
            cursor: None
        }
    )
    .await
    .is_err());
    assert!(store
        .write_discussion(write(
            "new",
            json!({"kind":"start","role":"user","body":"Should refuse"})
        ))
        .is_err());
}

#[tokio::test]
async fn complete_cache_rebuild_retains_resolved_groups_and_outcomes() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let a = start(&store, "a");
    let resolved = store
        .write_discussion(status(&a, "resolve", "resolved"))
        .unwrap();
    store
        .save_requirement_from_discussion(
            save(&store, "edit", json!({"description":"After"})),
            provenance_core::threads::DiscussionOrigin {
                thread_id: a.thread_id,
                discussion_id: a.discussion_id,
                message_id: a.message_id.unwrap(),
            },
        )
        .unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(root);
    let query = DiscussionQuery {
        parent: a.parent,
        limit: 50,
        cursor: None,
    };
    let first = read_discussions(root, &scope(), ReadPolicy::default(), query.clone())
        .await
        .unwrap();
    std::fs::remove_dir_all(layout.cache_dir()).unwrap();
    let rebuilt = read_discussions(root, &scope(), ReadPolicy::default(), query)
        .await
        .unwrap();
    assert_eq!(first.result.entries, rebuilt.result.entries);
    assert!(
        matches!(&rebuilt.result.entries[0],DiscussionGroup::Addressed {discussion,..} if **discussion==resolved)
    );
}

#[tokio::test]
async fn oversized_discussion_messages_are_refused_before_publication() {
    let (_temp, store) = fixture();
    // The encoded Message of this body exceeds the page record budget, so the
    // write must fail before anything is staged or published. A record that
    // the bounded page reader refuses would otherwise be unwritable-by-read:
    // persisted once and unreadable forever.
    let refused = store.write_discussion(write(
        "oversized",
        json!({"kind":"start","role":"user","body":"a".repeat(70_000)}),
    ));
    assert!(refused.is_err());
    assert!(store.list_messages(&scope()).unwrap().is_empty());
    assert!(store.list_threads(&scope()).unwrap().is_empty());
}

#[tokio::test]
async fn large_but_legal_discussion_messages_stay_readable() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    // 60,000 unescaped bytes plus the Message envelope stay under the
    // 65,536-byte record budget, so the bounded page reader must return them.
    let a = store
        .write_discussion(write(
            "large",
            json!({"kind":"start","role":"user","body":"a".repeat(60_000)}),
        ))
        .unwrap();
    let page = read_discussion_messages(
        root,
        &scope(),
        ReadPolicy::default(),
        DiscussionMessagesQuery {
            parent: a.parent,
            selector: DiscussionSelector::Discussion {
                discussion_id: a.discussion_id,
            },
            limit: 50,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(page.result.entries.len(), 1);
    assert_eq!(page.result.entries[0].body.len(), 60_000);
}

#[tokio::test]
async fn legacy_message_appends_refuse_oversized_bodies() {
    let (_temp, store) = fixture();
    let refused = store.post_thread_message(
        serde_json::from_value(json!({
            "scope_id":"default",
            "parent":{"node_type":"requirement","node_id":"req_a"},
            "role":"user",
            "body":"a".repeat(70_000)
        }))
        .unwrap(),
    );
    assert!(refused.is_err());
    assert!(store.list_messages(&scope()).unwrap().is_empty());
    store
        .post_thread_message(
            serde_json::from_value(json!({
                "scope_id":"default",
                "parent":{"node_type":"requirement","node_id":"req_a"},
                "role":"user",
                "body":"The system keeps messages readable."
            }))
            .unwrap(),
        )
        .unwrap();
}
