use crate::repository::IdSource;
use provenance_core::StableId;
use provenance_macros::rule;

/// A new Message identity: `msg_` and a UUID from the ID source of the
/// store. The default ID source gives random UUIDs.
#[rule("rule_message_id_is_random")]
pub fn new_message_id(ids: &dyn IdSource) -> StableId {
    StableId::new(format!("msg_{}", ids.new_uuid())).expect("a prefixed UUID is a stable ID")
}

/// Message times follow the write clock and increase within a scope.
///
/// A time past the range of the read database, ±(2^53 − 1), is refused.
#[rule("rule_message_time_follows_write_clock")]
pub fn next_message_time(now_ms: i64, latest: Option<i64>) -> anyhow::Result<i64> {
    let time = match latest {
        Some(latest) if latest >= now_ms => latest.checked_add(1),
        _ => Some(now_ms),
    };
    time.filter(|time| provenance_core::model::exact_integer::is_exact(*time))
        .ok_or_else(|| crate::refusal::fault!("Message timestamp overflow"))
}

/// A line of a shard file. The first line is 1.
pub struct ShardLine {
    pub path: camino::Utf8PathBuf,
    pub line: usize,
}

impl ShardLine {
    pub fn new(path: &camino::Utf8Path, line: usize) -> Self {
        Self {
            path: path.to_owned(),
            line,
        }
    }
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
            .map(|location| {
                format!(
                    "{}:{}",
                    super::readers::state_file(&location.path),
                    location.line
                )
            })
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
