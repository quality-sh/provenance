use crate::repository::IdSource;
use provenance_core::StableId;
use provenance_macros::rule;

/// A new Message identity from the ID source of the store.
pub fn new_message_id(ids: &dyn IdSource) -> StableId {
    StableId::new(format!("msg_{}", ids.new_uuid())).expect("a prefixed UUID is a stable ID")
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

pub struct ShardLine {
    pub path: camino::Utf8PathBuf,
    pub line: usize,
}

/// Duplicate Message identities are refused with every stored row location.
#[rule("rule_duplicate_message_id_names_lines")]
pub fn ensure_unique_message_ids(
    rows: &[(provenance_core::Message, ShardLine)],
) -> anyhow::Result<()> {
    let mut locations = std::collections::BTreeMap::<&str, Vec<&ShardLine>>::new();
    for (message, location) in rows {
        locations
            .entry(message.id.as_str())
            .or_default()
            .push(location);
    }
    if let Some((id, locations)) = locations
        .into_iter()
        .find(|(_, locations)| locations.len() > 1)
    {
        let lines = locations
            .iter()
            .map(|location| format!("{}:{}", location.path, location.line))
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("Message ID {id} appears more than once: {lines}; see docs/cli.md#repairing-a-duplicate-message-id");
    }
    Ok(())
}
