//! Dev-build-only capture of agent pain-point notes about provenance itself.
//!
//! Design constraints (see docs/dogfood.md):
//! - Capture is local-only: notes append to a JSONL spool on this machine.
//!   There is no network path, no endpoint, and nothing to authenticate.
//! - The note carries the feedback plus a session-id join key; everything a
//!   sister system (e.g. workflowd) already knows about the session arrives
//!   by enrichment at report time, never by agent self-report.
//! - Capture must not fail on missing context: absent session env or a
//!   non-git cwd degrade to nulls, never to errors.

use anyhow::Context;
use provenance_porcelain::dogfood::{
    Category, DogfoodListInput, DogfoodNoteInput, DogfoodPort, DogfoodReportInput,
    DogfoodSimulateInput, Enrichment, Note, NoteRecorded, Report, ReportBucket, ReportNote,
    Severity, Simulation, TriageListInput, TriageMarked, TriageView,
};
use provenance_porcelain::WorkFuture;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

const ENRICHMENT_CONTRACT: &str = "provenance-dogfood-enrichment/v1";
const SESSION_ENV_VARS: &[&str] = &[
    "PROVENANCE_SESSION_ID",
    "WORKFLOWD_SESSION_ID",
    "CLAUDE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ID",
    "OPENCODE_SESSION_ID",
];

mod simulate;
mod triage;

/// The shape a sister system provides at report time: session ids mapped to
/// whatever ground truth it holds about them (harness, model, machine, ...).
/// Fields inside each session object are passed through untouched so the
/// contract can grow without a provenance release.
#[derive(Deserialize)]
struct EnrichmentFile {
    contract: String,
    sessions: BTreeMap<String, serde_json::Value>,
}

/// The adapter of the dogfood commands: a local spool of notes and a sandbox runner.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpoolPort;

impl DogfoodPort for SpoolPort {
    type Failure = anyhow::Error;

    fn note(&self, input: DogfoodNoteInput) -> WorkFuture<'_, NoteRecorded, anyhow::Error> {
        Box::pin(async move { note(input) })
    }

    fn list(&self, _input: DogfoodListInput) -> WorkFuture<'_, Vec<Note>, anyhow::Error> {
        Box::pin(async move { read_spool() })
    }

    fn report(
        &self,
        _input: DogfoodReportInput,
        enrichment: Enrichment,
    ) -> WorkFuture<'_, Report, anyhow::Error> {
        Box::pin(async move { report(&enrichment) })
    }

    fn triage_mark(
        &self,
        id: String,
        handled: bool,
        reason: Option<String>,
    ) -> WorkFuture<'_, TriageMarked, anyhow::Error> {
        Box::pin(async move { triage::set_state(&id, handled, reason) })
    }

    fn triage_list(
        &self,
        input: TriageListInput,
    ) -> WorkFuture<'_, Vec<TriageView>, anyhow::Error> {
        Box::pin(async move { triage::list(input.status) })
    }

    fn simulate(&self, input: DogfoodSimulateInput) -> WorkFuture<'_, Simulation, anyhow::Error> {
        Box::pin(async move {
            let dir = input.dir.map(camino::Utf8PathBuf::from);
            tokio::task::spawn_blocking(move || simulate::run(input.keep, dir.as_deref()))
                .await?
                .map(|text| Simulation { text })
        })
    }
}

fn note(input: DogfoodNoteInput) -> anyhow::Result<NoteRecorded> {
    let record = Note {
        ts_ms: now_ms(),
        session_id: session_id_from_env(),
        host: hostname(),
        repo: git_context(&["rev-parse", "--show-toplevel"]),
        branch: git_context(&["branch", "--show-current"]),
        commit: git_context(&["rev-parse", "HEAD"]),
        provenance_version: env!("CARGO_PKG_VERSION").to_string(),
        surface: input.surface,
        category: input.category,
        severity: input.severity,
        summary: input.summary,
        detail: input.detail,
        suggestion: input.suggestion,
    };

    let dir = spool_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating dogfood spool dir {}", dir.display()))?;
    let path = dir.join("notes.jsonl");
    let mut line = serde_json::to_string(&record)?;
    line.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening dogfood spool {}", path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("appending dogfood note to {}", path.display()))?;

    Ok(NoteRecorded {
        spool: path.display().to_string(),
    })
}

fn report(enrichment: &Enrichment) -> anyhow::Result<Report> {
    let notes = read_spool()?;
    let sessions = load_enrichment(enrichment)?;

    let mut buckets: BTreeMap<(String, Category, Severity), usize> = BTreeMap::new();
    for note in &notes {
        *buckets
            .entry((note.surface.clone(), note.category, note.severity))
            .or_default() += 1;
    }

    let report = Report {
        total: notes.len(),
        counts: buckets
            .into_iter()
            .map(|((surface, category, severity), count)| ReportBucket {
                surface,
                category,
                severity,
                count,
            })
            .collect(),
        notes: notes
            .into_iter()
            .map(|note| {
                let session = sessions
                    .as_ref()
                    .zip(note.session_id.as_ref())
                    .and_then(|(sessions, id)| sessions.get(id).cloned());
                ReportNote { note, session }
            })
            .collect(),
    };
    Ok(report)
}

fn load_enrichment(
    enrichment: &Enrichment,
) -> anyhow::Result<Option<BTreeMap<String, serde_json::Value>>> {
    let raw = match enrichment {
        Enrichment::None => return Ok(None),
        Enrichment::Text(text) => text.clone(),
        Enrichment::File(path) => std::fs::read_to_string(path)
            .with_context(|| format!("reading enrichment file {path}"))?,
    };
    let file: EnrichmentFile = serde_json::from_str(&raw).context("parsing enrichment JSON")?;
    if file.contract != ENRICHMENT_CONTRACT {
        anyhow::bail!(
            "unsupported enrichment contract `{}`; expected `{ENRICHMENT_CONTRACT}`",
            file.contract
        );
    }
    Ok(Some(file.sessions))
}

fn read_spool() -> anyhow::Result<Vec<Note>> {
    let path = spool_dir().join("notes.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading dogfood spool {}", path.display()))?;
    // A torn write or stale line must not brick the review channel: skip
    // unreadable lines with a warning instead of refusing to aggregate.
    let mut notes = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(note) => notes.push(note),
            Err(err) => eprintln!(
                "skipping malformed dogfood note at {}:{}: {err}",
                path.display(),
                index + 1
            ),
        }
    }
    Ok(notes)
}

fn spool_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PROVENANCE_DOGFOOD_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    // Capture must not fail for lack of context: no HOME means the spool
    // degrades to the temp dir rather than erroring out.
    crate::skills::home_dir().map_or_else(
        |_| std::env::temp_dir().join("provenance-dogfood"),
        |home| home.join(".provenance").join("dogfood"),
    )
}

fn session_id_from_env() -> Option<String> {
    SESSION_ENV_VARS
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .filter(|value| !value.is_empty())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn hostname() -> String {
    if let Ok(host) = std::env::var("HOSTNAME") {
        if !host.is_empty() {
            return host;
        }
    }
    if let Ok(host) = std::fs::read_to_string("/etc/hostname") {
        let host = host.trim();
        if !host.is_empty() {
            return host.to_string();
        }
    }
    "unknown".to_string()
}

fn git_context(args: &[&str]) -> Option<String> {
    let output = provenance_store::git_process::git()
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
