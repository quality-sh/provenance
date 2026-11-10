use super::message_ids::{ensure_unique_message_ids, ShardLine};
use super::{Message, ProvenanceLayout, ScopeId, StateEntry, StateStore};
use crate::shards;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use serde::de::DeserializeOwned;

/// A stored line that is not one record of its family.
///
/// `serde_json` reports a position inside the one line it was given, which
/// reads as "line 1" for every row; the shard path and the row number are
/// what locate the fault. The display gives the path below the state
/// directory, so check output and a refusal name the same place.
#[derive(Debug, thiserror::Error)]
#[error("{} line {line}: {message}", state_relative(path))]
pub struct StateLineInvalid {
    pub path: Utf8PathBuf,
    pub line: u64,
    pub message: String,
}

impl StateLineInvalid {
    /// The shard path below the state directory, or only its file name when
    /// the shard is not below a state directory. A refusal that leaves the
    /// host gives this value and never an absolute path.
    pub fn file(&self) -> String {
        below_state(&self.path)
            .or_else(|| self.path.file_name().map(str::to_owned))
            .unwrap_or_default()
    }

    /// The first unparsable state line in the cause chain of `error`.
    pub fn find(error: &anyhow::Error) -> Option<&Self> {
        error.chain().find_map(|cause| cause.downcast_ref::<Self>())
    }
}

/// Names the shard and its line when a stored line is not one JSON record.
pub fn located(path: &Utf8Path, line_number: usize, error: &serde_json::Error) -> anyhow::Error {
    let text = error.to_string();
    let message = text
        .rfind(" at line ")
        .map_or(text.as_str(), |at| &text[..at]);
    invalid_line(path, line_number, message.to_owned())
}

fn invalid_line(path: &Utf8Path, line: usize, message: String) -> anyhow::Error {
    StateLineInvalid {
        path: path.to_owned(),
        line: u64::try_from(line).unwrap_or(u64::MAX),
        message,
    }
    .into()
}

/// The shard path below the state directory, with `/` between its parts on
/// every platform. A shard path joins native separators to a family suffix
/// that has `/`, so the plain display mixes the two on Windows.
fn state_relative(path: &Utf8Path) -> String {
    below_state(path).unwrap_or_else(|| path.to_string())
}

fn below_state(path: &Utf8Path) -> Option<String> {
    let components: Vec<&str> = path.iter().collect();
    components
        .windows(2)
        .rposition(|pair| pair == [".provenance", "state"])
        .map(|at| components[at + 2..].join("/"))
}

pub fn parse_line(
    path: &Utf8Path,
    line_number: usize,
    line: &str,
) -> anyhow::Result<serde_json::Value> {
    serde_json::from_str(line).map_err(|error| located(path, line_number, &error))
}

/// The one place a stored line becomes a record.
///
/// Every reader in this module lands here: [`read_records`] carries the
/// families, and [`read_message_shard_lines`] carries messages. A field that
/// the record type does not know is ignored. A family that does not pass
/// through this function is not read at all.
///
/// The caller has already turned the line into a [`serde_json::Value`],
/// because some of them look at it first.
fn record_from_line<T: DeserializeOwned>(
    path: &Utf8Path,
    line_number: usize,
    value: serde_json::Value,
) -> anyhow::Result<T> {
    ensure_no_legacy_retirement(path, line_number, &value)?;
    serde_json::from_value(value).map_err(|error| located(path, line_number, &error))
}

/// Refuses a stored record that still holds the removed `retired` field.
pub fn ensure_no_legacy_retirement(
    path: &Utf8Path,
    line_number: usize,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    let Some(record) = value.as_object() else {
        return Ok(());
    };
    if !record.contains_key("retired") {
        return Ok(());
    }
    let name = record
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| "record".to_string(), |id| format!("record {id}"));
    anyhow::bail!(
        "{path} line {line_number}: {name} contains legacy field `retired`; \
         canonical JSONL requires record-deletion migration 026 before this build can read it"
    )
}

/// The records of the shard at `path`, read through `store`; no records
/// when the shard does not exist.
fn read_records<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    let Some(contents) = store.state_text(path)? else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let value = parse_line(path, index + 1, line)?;
        records.push(record_from_line(path, index + 1, value)?);
    }
    Ok(records)
}

/// The records of the shard at `path`, each with its line, so that a check
/// over records that do not agree can name the record at fault.
pub fn read_jsonl_located<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<(T, ShardLine)>> {
    store.state_path_access(path, |held| {
        let Some(contents) = held.state_text(path)? else {
            return Ok(Vec::new());
        };
        let mut records = Vec::new();
        for (index, line) in contents.lines().enumerate() {
            let value = parse_line(path, index + 1, line)?;
            records.push((
                record_from_line(path, index + 1, value)?,
                ShardLine {
                    path: path.to_owned(),
                    line: index + 1,
                },
            ));
        }
        Ok(records)
    })
}

pub fn read_jsonl<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    store.state_path_access(path, |held| read_records(held, path))
}

/// Reads the shard at `path` through `store`, which holds the section.
pub(super) fn read_jsonl_held<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    read_records(store, path)
}

pub(super) fn read_ideation_landings<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    store.state_path_access(path, |held| read_records(held, path))
}

fn read_message_shard_lines<T: DeserializeOwned>(
    store: &StateStore,
    shard_paths: Vec<Utf8PathBuf>,
) -> anyhow::Result<Vec<(T, ShardLine)>> {
    let mut records = Vec::new();
    for path in shard_paths {
        let contents = store
            .read_state_text(&path)
            .with_context(|| format!("failed to read message shard {}", path.as_str()))?;
        for (index, line) in contents.lines().enumerate() {
            let value = parse_line(&path, index + 1, line)?;
            records.push((
                record_from_line(&path, index + 1, value)?,
                ShardLine {
                    path: path.clone(),
                    line: index + 1,
                },
            ));
        }
    }
    Ok(records)
}

pub(super) fn read_message_shards(
    store: &StateStore,
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Message>> {
    store.state_path_access(&shards::threads_path(layout, scope), |held| {
        read_message_shards_held(held, scope)
    })
}

pub(super) fn read_message_shards_held(
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Message>> {
    let rows = read_message_shard_lines(store, message_shard_paths_in(store, scope)?)?;
    ensure_unique_message_ids(&rows)?;
    Ok(rows.into_iter().map(|(message, _)| message).collect())
}

/// Every month shard of the scope's messages that `store` reads, sorted.
/// All message reads discover their shards here.
pub fn message_shard_paths_of(
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Utf8PathBuf>> {
    message_shard_paths_in(store, scope)
}

fn message_shard_paths_in(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<Utf8PathBuf>> {
    let threads_dir = shards::threads_path(&store.layout, scope)
        .parent()
        .expect("threads path must have a parent")
        .to_path_buf();
    let names = store.state_keys_under(&threads_dir, StateEntry::File, |path| {
        anyhow::anyhow!("non-UTF-8 message shard path: {}", path.display())
    })?;
    Ok(names
        .into_iter()
        .map(|name| threads_dir.join(name))
        .filter(|path| is_message_month_shard(path))
        .collect())
}

fn is_message_month_shard(path: &Utf8Path) -> bool {
    path.file_name()
        .is_some_and(shards::is_message_month_shard_name)
}

#[cfg(test)]
mod read_guard_tests;
