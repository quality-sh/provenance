//! What each discussion read gives over one fixed store: the entries,
//! their order, the pages that a cursor walks, and the refusals. The rows
//! are written straight into the read database, so their ids and times
//! are fixed. No Rule: the expected lines in `discussion_outputs.txt` hold
//! the behaviour while the discussion reads move onto the read database
//! port. On a difference the test prints every line it read.

use super::comparison::test_stores::TestStore;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::review::{
    read_discussion, read_discussion_conversation, read_discussion_list, read_discussion_message,
    read_discussion_messages, read_discussions,
};
use provenance_core::protocol::Stamped;
use provenance_core::threads::{
    DiscussionConversationQuery, DiscussionListQuery, DiscussionMessagesQuery, DiscussionQuery,
    DiscussionSelector, DiscussionStatusFilter,
};
use provenance_core::{NodeType, StableId, ThreadParent};
use serde::Serialize;

const EXPECTED: &str = include_str!("discussion_outputs.txt");

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn parent(id: &str) -> ThreadParent {
    ThreadParent {
        node_type: NodeType::Requirement,
        node_id: sid(id),
    }
}

fn policy() -> ReadPolicy {
    ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly)
}

/// The rows of three discussions, a legacy thread and their messages.
async fn seed(store: &TestStore) {
    let cache = open_cache(&store.layout()).await.unwrap();
    let pool = cache.pool();
    let run = |sql: &'static str, values: Vec<String>| async move {
        let mut query = sqlx::query(sql);
        for value in values {
            query = query.bind(value);
        }
        query.execute(pool).await.unwrap();
    };
    let thread =
        "INSERT INTO threads (scope_id, id, parent_type, parent_id, status, created_at, payload) \
                  VALUES ('default', ?, 'requirement', ?, ?, CAST(? AS INTEGER), '{}')";
    for row in [
        ["thread_one", "req_top", "active", "10"],
        ["thread_two", "req_top", "resolved", "20"],
        ["thread_three", "req_left", "active", "30"],
        ["thread_legacy", "req_top", "archived", "40"],
        ["thread_empty", "req_top", "active", "50"],
    ] {
        run(thread, row.map(String::from).to_vec()).await;
    }
    let message =
        "INSERT INTO messages (scope_id, id, thread_id, role, body, created_at, payload) \
                   VALUES ('default', ?, ?, ?, ?, CAST(? AS INTEGER), '{}')";
    let long = format!("{} end", "é".repeat(300));
    for row in [
        ["message_one_a", "thread_one", "user", "Opening one", "1"],
        ["message_one_c", "thread_one", "assistant", "Reply one", "2"],
        ["message_one_b", "thread_one", "user", "Reply two", "2"],
        ["message_two_a", "thread_two", "user", long.as_str(), "3"],
        [
            "message_three_a",
            "thread_three",
            "system",
            "Opening three",
            "4",
        ],
        [
            "message_legacy_b",
            "thread_legacy",
            "user",
            "Legacy two",
            "5",
        ],
        [
            "message_legacy_a",
            "thread_legacy",
            "user",
            "Legacy one",
            "5",
        ],
    ] {
        run(message, row.map(String::from).to_vec()).await;
    }
    let discussion = "INSERT INTO discussions (scope_id, discussion_id, parent, thread_id, \
                      root_message_id, message_ids, status, version, actor, outcomes, disposition_id) \
                      VALUES ('default', ?, ?, ?, ?, ?, ?, CAST(? AS INTEGER), 'ben', '[]', NULL)";
    for row in [
        [
            "discussion_b",
            r#"{"node_type":"requirement","node_id":"req_top"}"#,
            "thread_one",
            "message_one_a",
            r#"["message_one_a","message_one_c","message_one_b"]"#,
            "active",
            "3",
        ],
        [
            "discussion_a",
            r#"{"node_type":"requirement","node_id":"req_top"}"#,
            "thread_two",
            "message_two_a",
            r#"["message_two_a"]"#,
            "resolved",
            "2",
        ],
        [
            "discussion_c",
            r#"{"node_type":"requirement","node_id":"req_left"}"#,
            "thread_three",
            "message_three_a",
            r#"["message_three_a"]"#,
            "active",
            "1",
        ],
    ] {
        run(discussion, row.map(String::from).to_vec()).await;
    }
    cache.close().await.unwrap();
}

/// One line of an answer: the result as compact JSON with the cursor
/// shown only as present, or the refusal.
fn line<T: Serialize>(label: &str, answer: anyhow::Result<Stamped<T>>) -> String {
    match answer {
        Ok(stamped) => {
            let mut value = serde_json::to_value(&stamped.result).unwrap();
            if let Some(cursor) = value.get_mut("next_cursor").filter(|c| !c.is_null()) {
                *cursor = serde_json::Value::from("<cursor>");
            }
            if let Some(cursor) = value
                .pointer_mut("/messages/next_cursor")
                .filter(|c| !c.is_null())
            {
                *cursor = serde_json::Value::from("<cursor>");
            }
            format!("{label}: {value} attested {:?}", stamped.stamp.attested)
        }
        Err(error) => format!("{label}: refused: {error:#}"),
    }
}

fn next_cursor<T: Serialize>(answer: &anyhow::Result<Stamped<T>>) -> Option<String> {
    let value = serde_json::to_value(&answer.as_ref().ok()?.result).ok()?;
    let cursor = value
        .get("next_cursor")
        .or_else(|| value.pointer("/messages/next_cursor"))?;
    cursor.as_str().map(str::to_owned)
}

async fn groups(store: &TestStore, lines: &mut Vec<String>) {
    for (owner, limit) in [
        ("req_top", 1),
        ("req_top", 50),
        ("req_left", 2),
        ("req_none", 5),
    ] {
        let mut cursor = None;
        for page in 0..5 {
            let answer = read_discussions(
                &store.repository(),
                policy(),
                DiscussionQuery {
                    parent: parent(owner),
                    limit,
                    cursor: cursor.clone(),
                },
            )
            .await;
            cursor = next_cursor(&answer);
            lines.push(line(&format!("groups {owner} {limit} page {page}"), answer));
            if cursor.is_none() {
                break;
            }
        }
    }
    for (owner, id) in [
        ("req_top", "discussion_b"),
        ("req_top", "discussion_c"),
        ("req_top", "discussion_none"),
        ("req_none", "discussion_b"),
    ] {
        let answer = read_discussion(&store.repository(), policy(), parent(owner), sid(id)).await;
        lines.push(line(&format!("group {owner} {id}"), answer));
    }
}

fn selectors() -> Vec<(&'static str, &'static str, DiscussionSelector)> {
    let discussion = |id: &str| DiscussionSelector::Discussion {
        discussion_id: sid(id),
    };
    let legacy = |id: &str| DiscussionSelector::Legacy { thread_id: sid(id) };
    vec![
        ("req_top", "discussion_b", discussion("discussion_b")),
        ("req_top", "discussion_c", discussion("discussion_c")),
        ("req_left", "discussion_c", discussion("discussion_c")),
        ("req_top", "legacy", legacy("thread_legacy")),
        ("req_top", "legacy one", legacy("thread_one")),
        ("req_left", "legacy", legacy("thread_legacy")),
    ]
}

async fn messages(store: &TestStore, lines: &mut Vec<String>) {
    for (owner, label, selector) in selectors() {
        for limit in [1, 50] {
            let mut cursor = None;
            for page in 0..5 {
                let answer = read_discussion_messages(
                    &store.repository(),
                    policy(),
                    DiscussionMessagesQuery {
                        parent: parent(owner),
                        selector: selector.clone(),
                        limit,
                        cursor: cursor.clone(),
                    },
                )
                .await;
                cursor = next_cursor(&answer);
                lines.push(line(
                    &format!("messages {owner} {label} {limit} page {page}"),
                    answer,
                ));
                if cursor.is_none() {
                    break;
                }
            }
        }
        for id in ["message_one_b", "message_legacy_a", "message_none"] {
            let answer = read_discussion_message(
                &store.repository(),
                policy(),
                parent(owner),
                selector.clone(),
                sid(id),
            )
            .await;
            lines.push(line(&format!("message {owner} {label} {id}"), answer));
        }
    }
}

async fn discovery(store: &TestStore, lines: &mut Vec<String>) {
    let kinds = [
        ("requirements", vec![NodeType::Requirement]),
        ("rules", vec![NodeType::Rule]),
        ("none", Vec::new()),
    ];
    for (kind_label, kinds) in &kinds {
        for owner in [None, Some("req_top"), Some("req_none")] {
            for status in [
                DiscussionStatusFilter::All,
                DiscussionStatusFilter::Active,
                DiscussionStatusFilter::Resolved,
            ] {
                for limit in [1, 50] {
                    let mut cursor = None;
                    for page in 0..5 {
                        let answer = read_discussion_list(
                            &store.repository(),
                            policy(),
                            DiscussionListQuery {
                                parent: owner.map(parent),
                                allowed_parent_kinds: kinds.clone(),
                                status,
                                limit,
                                cursor: cursor.clone(),
                            },
                        )
                        .await;
                        cursor = next_cursor(&answer);
                        lines.push(line(
                            &format!("list {kind_label} {owner:?} {status:?} {limit} page {page}"),
                            answer,
                        ));
                        if cursor.is_none() {
                            break;
                        }
                    }
                }
            }
        }
        for id in ["discussion_b", "discussion_a", "discussion_none"] {
            let mut cursor = None;
            for page in 0..5 {
                let answer = read_discussion_conversation(
                    &store.repository(),
                    policy(),
                    DiscussionConversationQuery {
                        discussion_id: sid(id),
                        allowed_parent_kinds: kinds.clone(),
                        limit: 2,
                        cursor: cursor.clone(),
                    },
                )
                .await;
                cursor = next_cursor(&answer);
                lines.push(line(
                    &format!("conversation {kind_label} {id} page {page}"),
                    answer,
                ));
                if cursor.is_none() {
                    break;
                }
            }
        }
    }
}

fn compare(lines: &[String]) {
    let actual = lines.join("\n") + "\n";
    if actual != EXPECTED {
        println!("=== DISCUSSION OUTPUTS\n{actual}=== END DISCUSSION OUTPUTS");
        for (index, (left, right)) in actual.lines().zip(EXPECTED.lines()).enumerate() {
            assert_eq!(left, right, "line {}", index + 1);
        }
        assert_eq!(actual.lines().count(), EXPECTED.lines().count());
    }
}

#[tokio::test]
async fn each_discussion_read_gives_the_same_pages_order_and_refusals() {
    let store = TestStore::pinned();
    catch_up_state(&store.repository()).await.unwrap();
    seed(&store).await;
    let mut lines = Vec::new();
    groups(&store, &mut lines).await;
    messages(&store, &mut lines).await;
    discovery(&store, &mut lines).await;
    compare(&lines);
}
