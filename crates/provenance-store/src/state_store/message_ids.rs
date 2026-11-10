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
            .ok_or_else(|| crate::refusal::fault!("Message timestamp overflow")),
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
            .map(|location| format!("{}:{}", state_file(location), location.line))
            .collect::<Vec<_>>()
            .join(", ");
        // The refusal is at the second row with the id. The message gives
        // each row with its shard path below the state directory, because the
        // message reaches the caller as it is.
        return Err(crate::refusal::state_fault(
            locations[1],
            format!("Message ID {id} appears more than once: {lines}; see docs/cli.md#repairing-a-duplicate-message-id"),
        ));
    }
    Ok(())
}

/// The shard path of `location` below the state directory.
fn state_file(location: &ShardLine) -> String {
    super::readers::StateLineInvalid {
        path: location.path.clone(),
        line: 0,
        message: String::new(),
    }
    .file()
}
