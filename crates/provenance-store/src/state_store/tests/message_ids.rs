use super::initialized_store;
use provenance_core::{MessageRole, StableId};
use provenance_macros::verifies;

#[test]
#[verifies("rule_message_id_is_random", examples)]
#[verifies("rule_message_time_follows_write_clock", examples)]
fn independent_stores_write_distinct_uuid_messages_at_the_write_clock() {
    let (_a, a, scope) = initialized_store();
    let (_b, b, _) = initialized_store();
    let thread = StableId::new("thread_same").unwrap();
    let before = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    let first = a
        .append_discussion_message(&scope, &thread, MessageRole::User, "first".into())
        .unwrap();
    let other = b
        .append_discussion_message(&scope, &thread, MessageRole::User, "other".into())
        .unwrap();
    let after = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    assert_ne!(first.id, other.id);
    for message in [&first, &other] {
        let id = uuid::Uuid::parse_str(message.id.as_str().strip_prefix("msg_").unwrap()).unwrap();
        assert_eq!(id.get_version_num(), 4);
        assert!((before..=after).contains(&i128::from(message.created_at)));
    }
    let second = a
        .append_discussion_message(&scope, &thread, MessageRole::User, "second".into())
        .unwrap();
    assert!(second.created_at > first.created_at);
    let mut combined = a.list_messages(&scope).unwrap();
    combined.push(other);
    crate::jsonl::write_jsonl_atomic(&crate::shards::messages_path(&a.layout, &scope), &combined)
        .unwrap();
    assert_eq!(a.list_messages(&scope).unwrap().len(), 3);
}

#[test]
#[verifies("rule_message_time_follows_write_clock", property)]
fn generated_message_times_follow_the_clock_or_increase_the_latest() {
    use crate::state_store::message_ids::next_message_time;
    let mut seed = 0x1234_5678_90ab_cdef_u64;
    for _ in 0..10_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let clock = i64::from_ne_bytes(seed.to_ne_bytes());
        let latest = clock.saturating_add(i64::from(i8::from_ne_bytes([seed.to_ne_bytes()[7]])));
        for latest in [None, Some(latest), Some(i64::MAX)] {
            let result = next_message_time(clock, latest);
            if latest == Some(i64::MAX) {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("timestamp overflow"));
            } else {
                let time = result.unwrap();
                assert_eq!(time, latest.map_or(clock, |latest| clock.max(latest + 1)));
            }
        }
    }
}

fn posted_store() -> (
    tempfile::TempDir,
    crate::state_store::StateStore,
    provenance_core::ScopeId,
    provenance_core::Message,
) {
    use crate::state_store::PostMessageInput;
    use provenance_core::{NodeType, ThreadParent};
    let (dir, store, scope) = super::seeded_requirement_store();
    let message = store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: StableId::new("req_overtime").unwrap(),
            },
            role: MessageRole::User,
            body: "first".into(),
        })
        .unwrap()
        .message;
    (dir, store, scope, message)
}

#[test]
#[verifies("rule_duplicate_message_id_names_lines", examples)]
fn duplicate_message_rows_name_both_lines_and_refuse_a_write() {
    let (_dir, store, scope, first) = posted_store();
    let path = crate::shards::messages_path(&store.layout, &scope);
    let mut second = first.clone();
    second.body = "second".into();
    crate::jsonl::write_jsonl_atomic(&path, &[first.clone(), second]).unwrap();
    let before = std::fs::read(&path).unwrap();
    for error in [
        store.list_messages(&scope).unwrap_err(),
        store
            .append_discussion_message(
                &scope,
                &first.thread_id,
                MessageRole::User,
                "refused".into(),
            )
            .unwrap_err(),
    ] {
        let text = format!("{error:#}");
        assert!(
            text.contains(&format!(
                "Message ID {} appears more than once:",
                first.id.as_str()
            )),
            "{text}"
        );
        for line in [1, 2] {
            assert!(text.contains(&format!("{path}:{line}")), "{text}");
        }
        assert!(
            text.contains("docs/cli.md#repairing-a-duplicate-message-id"),
            "{text}"
        );
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
#[verifies("rule_duplicate_message_id_names_lines", examples)]
fn duplicate_message_rows_name_every_month_shard_line() {
    let (_dir, store, scope, first) = posted_store();
    let current = crate::shards::messages_path(&store.layout, &scope);
    let early = current.parent().unwrap().join("2000-01.jsonl");
    let later = current.parent().unwrap().join("2000-02.jsonl");
    let mut unique = first.clone();
    unique.id = StableId::new("msg_innocent").unwrap();
    crate::jsonl::write_jsonl_atomic(&early, &[unique, first.clone()]).unwrap();
    crate::jsonl::write_jsonl_atomic(&later, std::slice::from_ref(&first)).unwrap();
    let error = store.list_messages(&scope).unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains(first.id.as_str()), "{text}");
    for (path, line) in [(current, 1), (early, 2), (later, 1)] {
        assert!(text.contains(&format!("{path}:{line}")), "{text}");
    }
    assert!(!text.contains("msg_innocent"), "{text}");
}
