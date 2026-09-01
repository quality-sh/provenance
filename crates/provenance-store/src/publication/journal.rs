//! The catch-up journal: a work hint, never proof.
//!
//! Writers append one event per committed shard write, inside the
//! publication section that committed it. Catch-up drains events to learn
//! what to re-derive cheaply; every freshness claim still rests on hashing
//! complete shard bytes, so a lost, gapped, truncated, or absent journal
//! costs speed and never correctness.
//!
//! One monotonic sequence space covers events and the stored revision
//! serial. Three durable values bound it: the stored serial in the database, the
//! append-only tail (each line carries its sequence), and the head record —
//! the next sequence the allocator hands out. The head is derived state:
//! normalization repairs it from whichever components survive.

use crate::layout::ProvenanceLayout;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::io::Write;

/// One committed shard write, named by scope and declared family.
///
/// `scope` is empty for the global edges family. `record_id` may be empty
/// when the write seam does not know which record moved; catch-up re-derives
/// whole families, so the id is context, not a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEvent {
    pub sequence: i64,
    pub scope: String,
    pub family: String,
    pub record_id: String,
    pub operation: String,
}

fn journal_dir(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.cache_dir().join("journal")
}

fn events_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    journal_dir(layout).join("events.jsonl")
}

fn head_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    journal_dir(layout).join("head.json")
}

#[derive(Serialize, Deserialize)]
struct HeadRecord {
    schema_version: u32,
    next_sequence: i64,
}

pub fn read_head_record(layout: &ProvenanceLayout) -> anyhow::Result<Option<i64>> {
    let path = head_path(layout);
    if !path.exists() {
        return Ok(None);
    }
    let record: HeadRecord = match serde_json::from_str(&std::fs::read_to_string(&path)?) {
        Ok(record) => record,
        // An unreadable head is a lost component; normalization rebuilds it.
        Err(_) => return Ok(None),
    };
    Ok(Some(record.next_sequence))
}

fn write_head_record(layout: &ProvenanceLayout, next_sequence: i64) -> anyhow::Result<()> {
    let dir = journal_dir(layout);
    std::fs::create_dir_all(&dir)?;
    let record = HeadRecord {
        schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        next_sequence,
    };
    let mut temporary = tempfile::NamedTempFile::new_in(&dir)?;
    temporary.write_all(&serde_json::to_vec(&record)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(head_path(layout))?;
    super::sync_directory(&dir)
}

/// Reads the surviving tail, skipping lines that do not parse.
///
/// A malformed or truncated line narrows the drain hint; it cannot hide a
/// change, because catch-up hashes every family regardless.
fn read_tail(layout: &ProvenanceLayout) -> anyhow::Result<Vec<JournalEvent>> {
    let path = events_path(layout);
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(std::fs::read_to_string(&path)?
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

fn tail_high_water(layout: &ProvenanceLayout) -> anyhow::Result<i64> {
    Ok(read_tail(layout)?
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap_or(0))
}

/// Repairs the head from whichever durable components survive.
///
/// The normalized head is one plus the highest of the stored revision
/// serial, the head record minus one, and the tail high-water. The record is
/// persisted and fsynced when it moves. Runs only inside a held publication
/// section or guard scope.
pub fn normalize_head(layout: &ProvenanceLayout, stored_serial: i64) -> anyhow::Result<i64> {
    let recorded = read_head_record(layout)?.unwrap_or(1);
    let head = 1 + stored_serial
        .max(recorded - 1)
        .max(tail_high_water(layout)?);
    if head != read_head_record(layout)?.unwrap_or(0) {
        write_head_record(layout, head)?;
    }
    Ok(head)
}

/// Appends one event and advances the head, both fsynced, in that order.
///
/// A crash between the append and the advance leaves the tail ahead of the
/// head; allocation and normalization both take the max of the two, so no
/// sequence is ever reused. Runs only inside a held publication section or
/// guard scope.
pub fn append_event(
    layout: &ProvenanceLayout,
    scope: &str,
    family: &str,
    record_id: &str,
    operation: &str,
) -> anyhow::Result<JournalEvent> {
    let sequence = read_head_record(layout)?
        .unwrap_or(1)
        .max(tail_high_water(layout)? + 1);
    let event = JournalEvent {
        sequence,
        scope: scope.to_string(),
        family: family.to_string(),
        record_id: record_id.to_string(),
        operation: operation.to_string(),
    };
    let dir = journal_dir(layout);
    std::fs::create_dir_all(&dir)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(layout))?;
    writeln!(file, "{}", serde_json::to_string(&event)?)?;
    file.sync_all()?;
    crate::test_probes::at("journal_event_appended")?;
    write_head_record(layout, sequence + 1)?;
    Ok(event)
}

/// Journals one committed shard write, keyed by declared family.
///
/// Runs inside the writer's held publication section, after the shard write
/// committed. A path outside the family table journals nothing.
pub(super) fn record_shard_write(
    layout: &ProvenanceLayout,
    path: &camino::Utf8Path,
) -> anyhow::Result<()> {
    crate::test_probes::at("writer_canonical_committed")?;
    if let Some((family, scope)) = crate::cache::family_for_shard_path(layout, path) {
        append_event(
            layout,
            scope.as_ref().map_or("", provenance_core::ScopeId::as_str),
            family.family_name(),
            "",
            "mutate",
        )?;
    }
    Ok(())
}

/// The drain hint for one pass: surviving events inside the bounds.
pub fn events_in_window(
    layout: &ProvenanceLayout,
    from: i64,
    to: i64,
) -> anyhow::Result<Vec<JournalEvent>> {
    let mut events: Vec<JournalEvent> = read_tail(layout)?
        .into_iter()
        .filter(|event| event.sequence >= from && event.sequence <= to)
        .collect();
    events.sort_by_key(|event| event.sequence);
    Ok(events)
}

/// Drops every event at or below the committed serial.
///
/// Runs after the pass committed and re-fsynced the head, so the tail holds
/// only sequences above the stored revision.
pub fn prune_up_to(layout: &ProvenanceLayout, serial: i64) -> anyhow::Result<()> {
    let path = events_path(layout);
    if !path.exists() {
        return Ok(());
    }
    let kept: Vec<JournalEvent> = read_tail(layout)?
        .into_iter()
        .filter(|event| event.sequence > serial)
        .collect();
    let dir = journal_dir(layout);
    let mut temporary = tempfile::NamedTempFile::new_in(&dir)?;
    for event in &kept {
        writeln!(temporary, "{}", serde_json::to_string(event)?)?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(&path)?;
    super::sync_directory(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ProvenanceLayout;

    fn layout() -> (tempfile::TempDir, ProvenanceLayout) {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        (dir, ProvenanceLayout::new(root))
    }

    fn event(layout: &ProvenanceLayout) -> JournalEvent {
        append_event(layout, "default", "requirements", "req_a", "mutate").unwrap()
    }

    #[test]
    fn append_allocates_monotonic_sequences_and_advances_the_head() {
        let (_dir, layout) = layout();
        assert_eq!(event(&layout).sequence, 1);
        assert_eq!(event(&layout).sequence, 2);
        assert_eq!(read_head_record(&layout).unwrap(), Some(3));
    }

    #[test]
    fn normalization_starts_a_fresh_space_at_one() {
        let (_dir, layout) = layout();
        assert_eq!(normalize_head(&layout, 0).unwrap(), 1);
    }

    #[test]
    fn normalization_takes_the_stored_serial_as_a_floor() {
        let (_dir, layout) = layout();
        assert_eq!(normalize_head(&layout, 7).unwrap(), 8);
        assert_eq!(read_head_record(&layout).unwrap(), Some(8));
    }

    #[test]
    fn normalization_repairs_a_tail_that_ran_ahead_of_the_head() {
        let (_dir, layout) = layout();
        event(&layout);
        event(&layout);
        write_head_record(&layout, 2).unwrap();
        assert_eq!(normalize_head(&layout, 0).unwrap(), 3);
    }

    #[test]
    fn normalization_keeps_the_head_after_a_prune_emptied_the_tail() {
        let (_dir, layout) = layout();
        for _ in 0..5 {
            event(&layout);
        }
        prune_up_to(&layout, 5).unwrap();
        assert_eq!(events_in_window(&layout, 1, i64::MAX).unwrap().len(), 0);
        assert_eq!(normalize_head(&layout, 3).unwrap(), 6);
    }

    #[test]
    fn window_returns_only_events_inside_the_bounds() {
        let (_dir, layout) = layout();
        for _ in 0..4 {
            event(&layout);
        }
        let window = events_in_window(&layout, 2, 3).unwrap();
        assert_eq!(
            window
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(window[0].family, "requirements");
        assert_eq!(window[0].scope, "default");
    }

    #[test]
    fn a_malformed_tail_line_is_skipped_not_fatal() {
        let (_dir, layout) = layout();
        event(&layout);
        let path = events_path(&layout);
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("{malformed\n");
        std::fs::write(&path, content).unwrap();
        assert_eq!(events_in_window(&layout, 1, i64::MAX).unwrap().len(), 1);
        assert_eq!(normalize_head(&layout, 0).unwrap(), 2);
    }

    #[test]
    fn prune_drops_sequences_at_or_below_the_serial_and_keeps_the_rest() {
        let (_dir, layout) = layout();
        for _ in 0..4 {
            event(&layout);
        }
        prune_up_to(&layout, 2).unwrap();
        assert_eq!(
            events_in_window(&layout, 1, i64::MAX)
                .unwrap()
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![3, 4]
        );
    }

    #[test]
    fn a_crash_between_append_and_head_advance_never_reuses_a_sequence() {
        let (_dir, layout) = layout();
        event(&layout);
        crate::test_probes::crash_at("journal_event_appended");
        let error = append_event(&layout, "default", "rules", "rule_a", "mutate").unwrap_err();
        assert!(error.to_string().contains("injected crash"));
        crate::test_probes::disarm("journal_event_appended");
        // The tail holds sequence 2; the head record still says 2. The next
        // allocation must skip to 3.
        assert_eq!(event(&layout).sequence, 3);
    }
}
