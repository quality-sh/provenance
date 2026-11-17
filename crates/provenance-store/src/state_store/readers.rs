use super::message_ids::{ensure_unique_message_ids, ShardLine};
use super::{Message, ProvenanceLayout, ScopeId, StateEntry, StateStore};
use crate::shards;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::protocol::read_failure::{shown_file, UnparsableStateFile};
use serde::de::DeserializeOwned;

/// The shard path below the state directory, or only its file name when the
/// shard is not below a state directory.
pub fn state_file(path: &Utf8Path) -> String {
    shown_file(path, None)
}

/// The refusal of the file at `path`, which does not parse at `line`.
///
/// The refusal gives a path that a person can find and never an absolute path.
/// Each file of a store is below `<root>/.provenance`, where `<root>` is the
/// root of the store, so the root of a file below a `.provenance` directory is
/// the parent of that directory. A file that no store holds, such as a
/// `merge-jsonl` input, is in the project of the working directory when it is
/// below the root of that project.
pub fn unparsable(
    path: &Utf8Path,
    line: u64,
    message: String,
    correction: Option<String>,
) -> UnparsableStateFile {
    let absolute = std::path::absolute(path)
        .ok()
        .and_then(|absolute| Utf8PathBuf::from_path_buf(absolute).ok());
    let path = absolute.as_deref().unwrap_or(path);
    let repository = store_root(path)
        .map(Utf8Path::to_path_buf)
        .or_else(working_project);
    UnparsableStateFile::new(path, repository.as_deref(), line, message, correction)
}

/// The root of the store that holds `path`: the parent of the nearest
/// `.provenance` directory above `path`.
fn store_root(path: &Utf8Path) -> Option<&Utf8Path> {
    path.ancestors()
        .find(|ancestor| ancestor.file_name() == Some(".provenance"))
        .and_then(Utf8Path::parent)
}

/// The project of the working directory, by the rule of a repository open.
fn working_project() -> Option<Utf8PathBuf> {
    let directory = std::env::current_dir().ok()?;
    let directory = Utf8PathBuf::from_path_buf(directory).ok()?;
    crate::layout::enclosing_project(&directory).map(Utf8Path::to_path_buf)
}

/// The parse messages of a message row whose `actor` is missing or blank.
const MESSAGE_ACTOR_FAULTS: [&str; 2] = ["missing field `actor`", "the field `actor` is blank"];

/// The step that corrects a message row whose `actor` is missing or blank.
const MESSAGE_ACTOR_CORRECTION: &str = "To correct this fault, add the field actor to this \
    line, with the actor that wrote the Message. If that actor is not known, use the actor \
    authoring.";

/// Names the shard and its line when a stored line is not one JSON record. A message row
/// with no actor or a blank actor gets the step that corrects that line.
pub fn located(path: &Utf8Path, line_number: usize, error: &serde_json::Error) -> anyhow::Error {
    let at = ShardLine::new(path, line_number);
    let message = parse_message(error);
    if is_message_month_shard(path) && MESSAGE_ACTOR_FAULTS.contains(&message.as_str()) {
        return crate::refusal::corrected_state_fault(&at, message, MESSAGE_ACTOR_CORRECTION);
    }
    crate::refusal::state_fault(&at, message)
}

/// The message of a serde error with no position, because the refusal gives
/// the line itself.
pub fn parse_message(error: &serde_json::Error) -> String {
    let text = error.to_string();
    text.rfind(" at line ")
        .map_or(text.as_str(), |at| &text[..at])
        .to_owned()
}

/// The records of the shard at `path`, read through `store`; no records
/// when the shard does not exist.
fn read_records<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    Ok(store
        .state_text(path)?
        .map(|contents| records_of(path, &contents))
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(|(record, _)| record)
        .collect())
}

/// The one loop over the lines of a shard: each line becomes a record with
/// its line, through the line reader that the writes also use. A line that
/// the reader refuses refuses the read.
fn records_of<T: DeserializeOwned>(
    path: &Utf8Path,
    contents: &str,
) -> anyhow::Result<Vec<(T, ShardLine)>> {
    crate::jsonl::numbered_lines(contents.as_bytes())
        .map(|(number, line)| {
            Ok((
                crate::jsonl::record_of_line(path, number, line)?,
                ShardLine::new(path, number),
            ))
        })
        .collect()
}

/// The records of the shard at `path`, each with its line, so that a check
/// over records that do not agree can name the record at fault.
pub fn read_jsonl_located<T: DeserializeOwned>(
    store: &StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<(T, ShardLine)>> {
    store.state_path_access(path, |held| {
        held.state_text(path)?
            .map_or_else(|| Ok(Vec::new()), |contents| records_of(path, &contents))
    })
}

/// The `id` member of a stored line, of any JSON type.
#[derive(serde::Deserialize)]
struct Identified {
    id: Option<serde_json::Value>,
}

/// A stored record of the shard at `path` that does not agree with itself or
/// with another record. The refusal is `invalid_state_file` at the second
/// line with the id `id` when the shard holds it twice, else at its only
/// line. The shard is read again only here, on the refusal path. When that
/// read fails or does not find the id, the refusal is `message` with no line.
/// The read finds the id in the `id` key of each line, so it does not find a
/// line that holds the id under an alias of that key.
pub fn record_fault(
    store: &StateStore,
    path: &Utf8Path,
    id: &str,
    message: String,
) -> anyhow::Error {
    // An error of the second read is not given: it would replace the fault.
    let rows = read_jsonl_located::<Identified>(store, path).unwrap_or_default();
    let mut lines = rows
        .into_iter()
        .filter(|(row, _)| row.id.as_ref().and_then(serde_json::Value::as_str) == Some(id))
        .map(|(_, line)| line);
    let first = lines.next();
    match lines.next().or(first) {
        Some(at) => crate::refusal::state_fault(&at, message),
        None => crate::refusal::fault!("{message}"),
    }
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

fn read_message_shard_lines<T: DeserializeOwned>(
    store: &StateStore,
    shard_paths: Vec<Utf8PathBuf>,
) -> anyhow::Result<Vec<(T, ShardLine)>> {
    let mut records = Vec::new();
    for path in shard_paths {
        let contents = store
            .read_state_text(&path)
            .with_context(|| format!("failed to read message shard {}", path.as_str()))?;
        records.extend(records_of(&path, &contents)?);
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
        crate::refusal::fault!("non-UTF-8 message shard path: {}", path.display())
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
