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
