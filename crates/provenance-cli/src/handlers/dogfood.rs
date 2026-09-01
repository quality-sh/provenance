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

use crate::cli::{Cli, DogfoodCategory, DogfoodCommand, DogfoodSeverity};
use crate::output::{self, OutputFormat};
use anyhow::Context;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

const ENRICHMENT_CONTRACT: &str = "provenance-dogfood-enrichment/v1";
const SESSION_ENV_VARS: &[&str] = &[
    "PROVENANCE_SESSION_ID",
    "WORKFLOWD_SESSION_ID",
    "CLAUDE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ID",
    "OPENCODE_SESSION_ID",
];

#[derive(Serialize, Deserialize)]
struct Note {
    ts_ms: i64,
    session_id: Option<String>,
    host: String,
    repo: Option<String>,
    branch: Option<String>,
    commit: Option<String>,
    provenance_version: String,
    surface: String,
    category: DogfoodCategory,
    severity: DogfoodSeverity,
    summary: String,
    detail: Option<String>,
    suggestion: Option<String>,
}

/// The shape a sister system provides at report time: session ids mapped to
/// whatever ground truth it holds about them (harness, model, machine, ...).
/// Fields inside each session object are passed through untouched so the
/// contract can grow without a provenance release.
#[derive(Deserialize)]
struct EnrichmentFile {
    contract: String,
    sessions: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize)]
struct ReportNote {
    #[serde(flatten)]
    note: Note,
    session: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ReportBucket {
    surface: String,
    category: DogfoodCategory,
    severity: DogfoodSeverity,
    count: usize,
}

#[derive(Serialize)]
struct Report {
    total: usize,
    counts: Vec<ReportBucket>,
    notes: Vec<ReportNote>,
}

pub(super) fn handle(command: DogfoodCommand, quiet: bool) -> anyhow::Result<()> {
    match command {
        DogfoodCommand::Note {
            surface,
            category,
            severity,
            detail,
            suggestion,
            summary,
        } => note(
            &surface, category, severity, summary, detail, suggestion, quiet,
        ),
        DogfoodCommand::List { format } => list(format),
        DogfoodCommand::Report { enrich, format } => report(enrich.as_ref(), format),
    }
}

fn note(
    surface: &str,
    category: DogfoodCategory,
    severity: DogfoodSeverity,
    summary: String,
    detail: Option<String>,
    suggestion: Option<String>,
    quiet: bool,
) -> anyhow::Result<()> {
    let surfaces = valid_surfaces();
    if !surfaces.iter().any(|s| s == surface) {
        anyhow::bail!(
            "unknown surface `{surface}`; valid surfaces: {}",
            surfaces.join(", ")
        );
    }

    let record = Note {
        ts_ms: now_ms(),
        session_id: session_id_from_env(),
        host: hostname(),
        repo: git_context(&["rev-parse", "--show-toplevel"]),
        branch: git_context(&["branch", "--show-current"]),
        commit: git_context(&["rev-parse", "HEAD"]),
        provenance_version: env!("CARGO_PKG_VERSION").to_string(),
        surface: surface.to_string(),
        category,
        severity,
        summary,
        detail,
        suggestion,
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

    if !quiet {
        println!("recorded dogfood note in {}", path.display());
    }
    Ok(())
}

fn list(format: OutputFormat) -> anyhow::Result<()> {
    let notes = read_spool()?;
    output::print(format, &notes)?;
    Ok(())
}

fn report(enrich: Option<&Utf8PathBuf>, format: OutputFormat) -> anyhow::Result<()> {
    let notes = read_spool()?;
    let sessions = enrich.map(load_enrichment).transpose()?;

    let mut buckets: BTreeMap<(String, DogfoodCategory, DogfoodSeverity), usize> = BTreeMap::new();
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
    output::print(format, &report)?;
    Ok(())
}

fn load_enrichment(path: &Utf8PathBuf) -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    let raw = if path == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("reading enrichment from stdin")?;
        buffer
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading enrichment file {path}"))?
    };
    let file: EnrichmentFile = serde_json::from_str(&raw).context("parsing enrichment JSON")?;
    if file.contract != ENRICHMENT_CONTRACT {
        anyhow::bail!(
            "unsupported enrichment contract `{}`; expected `{ENRICHMENT_CONTRACT}`",
            file.contract
        );
    }
    Ok(file.sessions)
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

/// Surfaces are the CLI's own top-level subcommand names plus "general",
/// derived from clap so the set never drifts from the actual command tree.
fn valid_surfaces() -> Vec<String> {
    let mut surfaces: Vec<String> = <Cli as clap::CommandFactory>::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();
    surfaces.push("general".to_string());
    surfaces.sort();
    surfaces
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
    let output = ProcessCommand::new("git").args(args).output().ok()?;
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
