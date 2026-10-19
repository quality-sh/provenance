use provenance_core::StableId;
use provenance_macros::rule;

/// New Message identities use random UUIDs so independent stores can merge them.
#[rule("rule_message_id_is_random")]
pub fn new_message_id() -> StableId {
    StableId::new(format!("msg_{}", uuid::Uuid::new_v4())).expect("a prefixed UUID is a stable ID")
}

/// Message times follow the write clock and increase within a scope.
#[rule("rule_message_time_follows_write_clock")]
pub fn next_message_time(now_ms: i64, latest: Option<i64>) -> anyhow::Result<i64> {
    match latest {
        Some(latest) if latest >= now_ms => latest
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Message timestamp overflow")),
        _ => Ok(now_ms),
    }
}
