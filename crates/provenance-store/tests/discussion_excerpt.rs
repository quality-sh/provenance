mod discussion_support;

use discussion_support::{fixture, scope, write};
use provenance_core::{threads::{DiscussionConversationQuery, DiscussionListQuery, DiscussionStatusFilter}, NodeType};
use provenance_store::{operations::read_policy::ReadPolicy, review::{read_discussion_conversation, read_discussion_list}};
use serde_json::json;

#[tokio::test]
async fn list_excerpt_counts_characters_across_nul_and_unicode() {
    let (temp, store) = fixture();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let cases = [
        ("nul_early", format!("abc\0{}", "x".repeat(300)), format!("abc\0{}", "x".repeat(236)), true),
        ("nul_at_limit", format!("{}\0y", "x".repeat(239)), format!("{}\0", "x".repeat(239)), true),
        ("nul_after_limit", format!("{}\0y", "x".repeat(240)), "x".repeat(240), true),
        ("unicode_nul", format!("{}\0{}", "é".repeat(100), "界".repeat(150)), format!("{}\0{}", "é".repeat(100), "界".repeat(139)), true),
        ("exact_limit", "a".repeat(240), "a".repeat(240), false),
        ("over_limit", "a".repeat(241), "a".repeat(240), true),
        ("short_unicode", "界".repeat(80), "界".repeat(80), false),
        ("exact_unicode", "界".repeat(240), "界".repeat(240), false),
        ("over_unicode", "界".repeat(241), "界".repeat(240), true),
    ];

    let ids = cases.iter().map(|(name, body, _, _)| {
        store.write_discussion(write(name, json!({"kind":"start", "role":"user", "body":body})))
            .unwrap().discussion_id
    }).collect::<Vec<_>>();
    let page = read_discussion_list(root, &scope(), ReadPolicy::default(), DiscussionListQuery {
        parent: None,
        allowed_parent_kinds: vec![NodeType::Requirement],
        status: DiscussionStatusFilter::Active,
        limit: 20,
        cursor: None,
    }).await.unwrap().result;
    assert_eq!(page.entries.len(), cases.len());
    assert!(page.next_cursor.is_none());
    for ((name, body, excerpt, truncated), id) in cases.iter().zip(ids) {
        let entry = page.entries.iter().find(|entry| entry.discussion_id == id).unwrap();
        assert_eq!(&entry.opening_excerpt, excerpt, "{name}");
        assert_eq!(entry.excerpt_truncated, *truncated, "{name}");
        if *name == "nul_early" {
            let conversation = read_discussion_conversation(root, &scope(), ReadPolicy::default(), DiscussionConversationQuery {
                discussion_id: entry.discussion_id.clone(),
                allowed_parent_kinds: vec![NodeType::Requirement],
                limit: 1,
                cursor: None,
            }).await.unwrap().result;
            assert_eq!(&conversation.messages.entries[0].body, body);
        }
    }
}
