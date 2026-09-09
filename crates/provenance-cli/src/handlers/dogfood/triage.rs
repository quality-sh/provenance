//! Dev-build-only triage state for captured dogfood notes.
//!
//! The note spool is capture history: append-only, never rewritten by
//! triage. State lives in a sibling `triage.jsonl` where every state change
//! is one append of one line, so concurrent triage commands cannot corrupt
//! each other's records and the last record for a note wins at read time. A
//! torn write only ever costs one line, skipped with a warning like the
//! spool reader does.
//!
//! A note's identifier is derived from the parsed note alone (an FNV-1a
//! digest of its canonical JSON). Notes captured before triage existed get
//! identifiers without any migration, and neither report enrichment nor a
//! position in a filtered list can change an identifier.

use super::{now_ms, read_spool, spool_dir, Note};
use crate::cli::{DogfoodCategory, DogfoodSeverity, TriageCommand, TriageFilter};
use crate::output::{self, OutputFormat};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

pub(super) fn handle(command: TriageCommand, quiet: bool) -> anyhow::Result<()> {
    match command {
        TriageCommand::Handle { id, reason } => set_state(&id, true, reason, quiet),
        TriageCommand::Reopen { id } => set_state(&id, false, None, quiet),
        TriageCommand::List { status, format } => list(status, format),
    }
}

/// One recorded state change for one note. Records are append-only; the
/// newest record for a note id is its current state.
#[derive(Serialize, Deserialize)]
struct StateRecord {
    ts_ms: i64,
    note_id: String,
    handled: bool,
    reason: Option<String>,
}

/// One note as the triage view shows it: the identifier to act on, the
/// current state, and enough of the note to recognize it.
#[derive(Serialize)]
struct TriageView {
    id: String,
    status: &'static str,
    reason: Option<String>,
    ts_ms: i64,
    session_id: Option<String>,
    surface: String,
    category: DogfoodCategory,
    severity: DogfoodSeverity,
    summary: String,
}

fn set_state(
    input: &str,
    handled: bool,
    reason: Option<String>,
    quiet: bool,
) -> anyhow::Result<()> {
    let notes = read_spool()?;
    let note_id = resolve_note_id(input, &notes)?;
    append_state(&StateRecord {
        ts_ms: now_ms(),
        note_id: note_id.clone(),
        handled,
        reason,
    })?;
    if !quiet {
        let status = if handled { "handled" } else { "unhandled" };
        println!("marked {note_id} {status}");
    }
    Ok(())
}

fn list(filter: TriageFilter, format: OutputFormat) -> anyhow::Result<()> {
    let notes = read_spool()?;
    let state = load_state()?;
    let mut views: Vec<TriageView> = notes.iter().map(|note| view(note, &state)).collect();
    views.sort_by(|a, b| a.id.cmp(&b.id));
    views.dedup_by(|a, b| a.id == b.id);
    let shown: Vec<TriageView> = views
        .into_iter()
        .filter(|entry| matches_filter(entry, filter))
        .collect();
    output::print(format, &shown)?;
    Ok(())
}

/// The stable identifier of a note: an FNV-1a digest of its canonical JSON.
/// It derives from the parsed note, so it is stable across invocations and
/// machines, identical logical notes share one identity regardless of how
/// their line was formatted, and rewriting state never shifts it.
fn note_id(note: &Note) -> String {
    let canonical = serde_json::to_vec(note).expect("canonical JSON of a note");
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in &canonical {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Resolve a user-supplied id or prefix to exactly one note id. Never
/// guesses: an identifier that matches nothing, or that matches several
/// notes, is an error that writes no state.
fn resolve_note_id(input: &str, notes: &[Note]) -> anyhow::Result<String> {
    let mut ids: Vec<String> = notes.iter().map(note_id).collect();
    ids.sort();
    ids.dedup();
    if ids.iter().any(|id| id == input) {
        return Ok(input.to_string());
    }
    let matches: Vec<String> = ids
        .iter()
        .filter(|id| id.starts_with(input))
        .cloned()
        .collect();
    match matches.as_slice() {
        [] if ids.is_empty() => {
            anyhow::bail!("no dogfood notes captured; nothing to resolve `{input}` against")
        }
        [] => anyhow::bail!(
            "unknown dogfood note id `{input}`; run `provenance dogfood triage list` for ids"
        ),
        [only] => Ok((*only).clone()),
        many => anyhow::bail!(
            "ambiguous dogfood note id prefix `{input}` matches {} notes: {}",
            many.len(),
            many.join(", ")
        ),
    }
}

fn view(note: &Note, state: &BTreeMap<String, StateRecord>) -> TriageView {
    let id = note_id(note);
    let (status, reason) = match state.get(&id) {
        Some(record) if record.handled => ("handled", record.reason.clone()),
        _ => ("unhandled", None),
    };
    TriageView {
        id,
        status,
        reason,
        ts_ms: note.ts_ms,
        session_id: note.session_id.clone(),
        surface: note.surface.clone(),
        category: note.category,
        severity: note.severity,
        summary: note.summary.clone(),
    }
}

fn matches_filter(entry: &TriageView, filter: TriageFilter) -> bool {
    match filter {
        TriageFilter::All => true,
        TriageFilter::Handled => entry.status == "handled",
        TriageFilter::Unhandled => entry.status == "unhandled",
    }
}

fn triage_path() -> PathBuf {
    spool_dir().join("triage.jsonl")
}

/// Load the current state: the last record per note id wins, and unreadable
/// lines are skipped with a warning so a torn write never blocks triage.
fn load_state() -> anyhow::Result<BTreeMap<String, StateRecord>> {
    let path = triage_path();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading dogfood triage state {}", path.display()))?;
    let mut state = BTreeMap::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<StateRecord>(line) {
            Ok(record) => {
                state.insert(record.note_id.clone(), record);
            }
            Err(err) => eprintln!(
                "skipping malformed dogfood triage record at {}:{}: {err}",
                path.display(),
                index + 1
            ),
        }
    }
    Ok(state)
}

fn append_state(record: &StateRecord) -> anyhow::Result<()> {
    let dir = spool_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating dogfood spool dir {}", dir.display()))?;
    let path = triage_path();
    let mut line = serde_json::to_string(record)?;
    line.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening dogfood triage state {}", path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("appending dogfood triage record to {}", path.display()))
}
