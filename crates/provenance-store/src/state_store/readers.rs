use super::message_ids::{ensure_unique_message_ids, ShardLine};
use super::{
    deserialize_closed, DispositionRecord, Message, ProvenanceLayout, ScopeId, StateEntry,
    StateStore,
};
use crate::shards;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use serde::de::DeserializeOwned;

/// What a reader does with fields its structs do not know.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fields {
    /// Unknown fields are ignored: the reader takes what it recognises.
    Open,
    /// Unknown fields are refused, so a pinned graph carries nothing extra.
    Closed,
}

/// Names the shard and its line when a stored line is not one JSON record.
///
/// `serde_json` reports a position inside the one line it was given, which
/// reads as "line 1" for every row; the shard path and the row number are
/// what locate the fault. The path is relative to the state directory, so the
/// message is safe to show on the review page.
pub fn located(path: &Utf8Path, line_number: usize, error: &serde_json::Error) -> anyhow::Error {
    let text = error.to_string();
    let message = text
        .rfind(" at line ")
        .map_or(text.as_str(), |at| &text[..at]);
    anyhow::anyhow!("{} line {line_number}: {message}", state_relative(path))
}

/// Locates a parse error from a deserializer that returns `anyhow`: a serde
/// error loses its in-line position, any other refusal gets the shard prefix.
fn located_any(path: &Utf8Path, line_number: usize, error: anyhow::Error) -> anyhow::Error {
    match error.downcast::<serde_json::Error>() {
        Ok(error) => located(path, line_number, &error),
        Err(error) => anyhow::anyhow!("{} line {line_number}: {error}", state_relative(path)),
    }
}

/// The shard path below the state directory, with `/` between its parts on
/// every platform. A shard path joins native separators to a family suffix
/// that has `/`, so the plain display mixes the two on Windows.
fn state_relative(path: &Utf8Path) -> String {
    let components: Vec<&str> = path.iter().collect();
    components
        .windows(2)
        .rposition(|pair| pair == [".provenance", "state"])
        .map_or_else(|| path.to_string(), |at| components[at + 2..].join("/"))
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
/// Every reader in this module lands here: [`read_records`] carries the open
/// families, the closed families and the aliased legacy dispositions;
/// [`read_message_shard_lines`] carries messages. A family that does not pass
/// through this function is not read at all.
///
/// The caller has already turned the line into a [`serde_json::Value`],
/// because some of them look at it first.
fn record_from_line<T: DeserializeOwned>(
    path: &Utf8Path,
    line_number: usize,
    line: &str,
    value: serde_json::Value,
    fields: Fields,
) -> anyhow::Result<T> {
    ensure_no_legacy_retirement(path, line_number, &value)?;
    match fields {
        Fields::Open => {
            serde_json::from_value(value).map_err(|error| located(path, line_number, &error))
        }
        // The closed reader needs the untouched line: `serde_ignored` reports
        // an unknown field by walking the document as it was written.
        Fields::Closed => {
            deserialize_closed(line).map_err(|error| located_any(path, line_number, error))
        }
    }
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

/// How a reader turns the lines of one shard into records.
struct Reading {
    fields: Fields,
    prepare: fn(&mut serde_json::Value),
}

const OPEN: Reading = Reading {
    fields: Fields::Open,
    prepare: leave_as_written,
};
const CLOSED: Reading = Reading {
    fields: Fields::Closed,
    ..OPEN
};
const LEGACY_DISPOSITIONS: Reading = Reading {
    prepare: normalize_disposition_aliases,
    ..OPEN
};

/// The records of the shard at `path`, read through `store`; no records
/// when the shard does not exist.
fn read_records<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
    reading: &Reading,
) -> anyhow::Result<Vec<T>> {
    let Some(contents) = store.state_text(path)? else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let mut value = parse_line(path, index + 1, line)?;
        (reading.prepare)(&mut value);
        records.push(record_from_line(
            path,
            index + 1,
            line,
            value,
            reading.fields,
        )?);
    }
    Ok(records)
}

const fn leave_as_written(_value: &mut serde_json::Value) {}

pub fn read_jsonl<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    store.state_path_access(path, || read_records(store, path, &OPEN))
}

/// Reads the shard at `path` through `store`, which holds the section.
pub(super) fn read_jsonl_held<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    read_records(store, path, &OPEN)
}

/// Reads the shard file at `path`; the caller holds the section.
pub(super) fn read_jsonl_unlocked<T: DeserializeOwned>(path: &Utf8Path) -> anyhow::Result<Vec<T>> {
    read_records(&disk(path), path, &OPEN)
}

pub(super) fn read_ideation_landings<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    store.state_path_access(path, || read_records(store, path, &OPEN))
}

pub(super) fn read_ideation_landings_unlocked<T: DeserializeOwned>(
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    read_records(&disk(path), path, &OPEN)
}

pub(super) fn read_legacy_dispositions(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<DispositionRecord>> {
    store.state_path_access(path, || read_records(store, path, &LEGACY_DISPOSITIONS))
}

pub(super) fn read_legacy_dispositions_unlocked(
    path: &Utf8Path,
) -> anyhow::Result<Vec<DispositionRecord>> {
    read_records(&disk(path), path, &LEGACY_DISPOSITIONS)
}

/// A store that reads files with no lock, for a caller that holds the
/// section. File reads use only the path, so the layout root is not used.
fn disk(path: &Utf8Path) -> StateStore {
    StateStore::read_only(ProvenanceLayout::new(path.to_owned()))
}

fn normalize_disposition_aliases(value: &mut serde_json::Value) {
    let Some(record) = value.as_object_mut() else {
        return;
    };
    rename_key(record, "promotionDecisionId", "id");
    rename_key(record, "proposalId", "proposal_id");
    rename_key(record, "decidedBy", "actor");
    rename_key(record, "canonicalArtifact", "canonical_artifact");
    if let Some(actor) = record
        .get_mut("actor")
        .and_then(serde_json::Value::as_object_mut)
    {
        rename_key(actor, "identityType", "identity_type");
        rename_key(actor, "userId", "id");
    }
    if let Some(artifact) = record
        .get_mut("canonical_artifact")
        .and_then(serde_json::Value::as_object_mut)
    {
        rename_key(artifact, "artifactType", "artifact_type");
        rename_key(artifact, "artifactId", "artifact_id");
    }
}

fn rename_key(object: &mut serde_json::Map<String, serde_json::Value>, old: &str, new: &str) {
    if !object.contains_key(new) {
        if let Some(value) = object.remove(old) {
            object.insert(new.to_owned(), value);
        }
    }
}

pub(super) fn read_jsonl_closed<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    store.state_path_access(path, || read_records(store, path, &CLOSED))
}

fn read_message_shard_lines<T: DeserializeOwned>(
    store: &StateStore,
    shard_paths: Vec<Utf8PathBuf>,
) -> anyhow::Result<Vec<(T, ShardLine)>> {
    let mut records = Vec::new();
    for path in shard_paths {
        crate::test_probes::record_read(&path);
        let contents = store
            .read_state_text(&path)
            .with_context(|| format!("failed to read message shard {}", path.as_str()))?;
        for (index, line) in contents.lines().enumerate() {
            let value = parse_line(&path, index + 1, line)?;
            records.push((
                record_from_line(&path, index + 1, line, value, Fields::Open)?,
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
    store.state_path_access(&shards::threads_path(layout, scope), || {
        read_message_shards_held(store, scope)
    })
}

fn read_message_shards_held(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<Message>> {
    let rows = read_message_shard_lines(store, message_shard_paths_in(store, scope)?)?;
    ensure_unique_message_ids(&rows)?;
    Ok(rows.into_iter().map(|(message, _)| message).collect())
}

pub(super) fn read_message_shards_unlocked(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Message>> {
    read_message_shards_held(&StateStore::read_only(layout.clone()), scope)
}

/// Every month shard of the scope's messages, sorted. All message reads
/// discover their shards here.
pub fn message_shard_paths(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Utf8PathBuf>> {
    message_shard_paths_in(&StateStore::read_only(layout.clone()), scope)
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
