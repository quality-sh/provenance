//! The projection write journal.
//!
//! Writers record invalidation events in this journal inside the same
//! locked section that commits canonical state. The journal is a work
//! hint: it names what a catch-up pass should re-derive, and it can never
//! prove that unjournaled bytes are unchanged, because it cannot see
//! writes that bypass it. Correctness never depends on it; the byte-verify
//! sweep does.
//!
//! One monotonic serial space covers journal sequences and the stored
//! projection revision serial. Three durable values bound it: the stored
//! revision serial, the journal tail, and the sequence head record. Entry
//! normalization takes the highest of the three before any allocation, so
//! a crash in any window never reuses a number.

use crate::layout::ProvenanceLayout;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// One invalidation event, as writers record it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEvent {
    pub sequence: u64,
    pub scope: String,
    pub family: String,
    pub record_id: String,
    pub operation: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeadRecord {
    next_sequence: u64,
}

const DEFAULT_HEAD: u64 = 1;

fn write_head(layout: &ProvenanceLayout, next_sequence: u64) -> anyhow::Result<()> {
    std::fs::create_dir_all(layout.journal_dir())?;
    let record = HeadRecord { next_sequence };
    let mut temporary = tempfile::NamedTempFile::new_in(layout.journal_dir())?;
    temporary.write_all(&serde_json::to_vec(&record)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(layout.journal_head_path())?;
    Ok(())
}

/// Reads the durable sequence head record, absent meaning one.
pub fn journal_head(layout: &ProvenanceLayout) -> anyhow::Result<u64> {
    let path = layout.journal_head_path();
    if !path.exists() {
        return Ok(DEFAULT_HEAD);
    }
    let record: HeadRecord = serde_json::from_slice(&std::fs::read(&path)?)?;
    Ok(record.next_sequence)
}

fn read_tail(layout: &ProvenanceLayout) -> anyhow::Result<Vec<JournalEvent>> {
    let path = layout.journal_events_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    for line in std::fs::read_to_string(&path)?.lines() {
        if line.is_empty() {
            continue;
        }
        events.push(serde_json::from_str(line)?);
    }
    Ok(events)
}

fn tail_high_water(events: &[JournalEvent]) -> u64 {
    events.iter().map(|event| event.sequence).max().unwrap_or(0)
}

/// Repairs the sequence head before any allocation or coverage
/// calculation.
///
/// The normalized head is one plus the highest of the stored revision
/// serial, the head record minus one, and the tail high-water. The head
/// record is persisted and fsynced when it moves.
pub fn normalize_head(layout: &ProvenanceLayout, stored_serial: u64) -> anyhow::Result<u64> {
    let tail_high = tail_high_water(&read_tail(layout)?);
    let head_record = journal_head(layout)?;
    let normalized = (stored_serial + 1).max(head_record).max(tail_high + 1);
    if normalized != head_record || !layout.journal_head_path().exists() {
        write_head(layout, normalized)?;
    }
    Ok(normalized)
}

/// Maps a mutated canonical shard path to its (scope, family) event key.
///
/// Returns `None` for paths outside canonical state or families the
/// projection does not store; catch-up finds those by digest comparison
/// alone.
pub fn shard_event_key(layout: &ProvenanceLayout, path: &Utf8Path) -> Option<(String, String)> {
    let family_dir = path
        .strip_prefix(layout.scopes_dir())
        .ok()
        .and_then(|relative| {
            let mut parts = relative.components();
            let scope = parts.next()?.as_str().to_string();
            let family = parts.next()?.as_str().to_string();
            Some((scope, family))
        });
    if let Some((scope, family)) = family_dir {
        return Some((scope, family));
    }
    let edges = path
        .strip_prefix(layout.edges_dir())
        .ok()
        .map(|_| (String::new(), "edges".to_string()));
    edges
}

/// Appends events under freshly allocated sequences, advancing the head
/// record afterwards.
///
/// A crash between the append and the head advance leaves the tail ahead
/// of the head; entry normalization takes the max, so no sequence is
/// reused.
pub fn record_events(layout: &ProvenanceLayout, events: &[JournalEvent]) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(layout.journal_dir())?;
    let head = journal_head(layout)?;
    let mut serialized = String::new();
    let mut sequence = head;
    for mut event in events.iter().cloned() {
        event.sequence = sequence;
        serialized.push_str(&serde_json::to_string(&event)?);
        serialized.push('\n');
        sequence += 1;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(layout.journal_events_path())?;
    file.write_all(serialized.as_bytes())?;
    file.sync_all()?;
    write_head(layout, sequence)?;
    Ok(())
}

/// Drains the events in the window `stored_serial + 1 ..= head - 1`.
///
/// While the caller holds the publication guard, writers cannot append,
/// so the window computed here is complete for the pass.
pub fn drain_window(
    layout: &ProvenanceLayout,
    stored_serial: u64,
    head: u64,
) -> anyhow::Result<Vec<JournalEvent>> {
    let floor = stored_serial.saturating_add(1);
    let ceiling = head.saturating_sub(1);
    Ok(read_tail(layout)?
        .into_iter()
        .filter(|event| event.sequence >= floor && event.sequence <= ceiling)
        .collect())
}

/// Removes drained events at or below `serial`, keeping the tail above it.
pub fn prune_through(layout: &ProvenanceLayout, serial: u64) -> anyhow::Result<()> {
    let path: Utf8PathBuf = layout.journal_events_path();
    if !path.exists() {
        return Ok(());
    }
    let remaining: Vec<JournalEvent> = read_tail(layout)?
        .into_iter()
        .filter(|event| event.sequence > serial)
        .collect();
    let mut serialized = String::new();
    for event in remaining {
        serialized.push_str(&serde_json::to_string(&event)?);
        serialized.push('\n');
    }
    let mut temporary = tempfile::NamedTempFile::new_in(layout.journal_dir())?;
    temporary.write_all(serialized.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path)?;
    Ok(())
}
