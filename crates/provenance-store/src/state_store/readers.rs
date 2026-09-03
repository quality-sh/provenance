use super::{DispositionRecord, Message, ProvenanceLayout, ScopeId};
use crate::shards;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::rule;
use serde::de::DeserializeOwned;

/// What a reader does with fields its structs do not know.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fields {
    /// Unknown fields are ignored: the reader takes what it recognises.
    Open,
    /// Unknown fields are refused, so a pinned graph carries nothing extra.
    Closed,
}

const NO_NESTED_RECORDS: &[&str] = &[];
const IDEATION_LANDING_RECORD_FIELDS: &[&str] = &[
    "contributions",
    "synthesis_packets",
    "proposals",
    "assertions",
    "dispositions",
];

/// The one place a stored line becomes a record.
///
/// Every reader in this module lands here: [`read_records`] carries the open
/// families, the closed families and the aliased legacy dispositions;
/// [`read_jsonl_shards`] carries messages. A family that does not pass
/// through this function is not read at all, which is what makes the version
/// guard below cover every record rather than the ones somebody remembered.
///
/// The caller has already turned the line into a [`serde_json::Value`],
/// because some of them look at it first.
fn record_from_line<T: DeserializeOwned>(
    path: &Utf8Path,
    line_number: usize,
    line: &str,
    value: serde_json::Value,
    fields: Fields,
    nested_record_fields: &[&str],
) -> anyhow::Result<T> {
    ensure_supported_record_version(path, line_number, &value)?;
    ensure_supported_nested_record_versions(path, line_number, &value, nested_record_fields)?;
    match fields {
        Fields::Open => Ok(serde_json::from_value(value)?),
        // The closed reader needs the untouched line: `serde_ignored` reports
        // an unknown field by walking the document as it was written.
        Fields::Closed => deserialize_closed(line),
    }
}

/// Nothing is loaded from disk except at the schema version this build reads.
///
/// A stored record says which layout it was written in. A later version means
/// a different layout: a field moved, dropped, or read with a new meaning.
/// Serde would take such a record on whatever fields still line up and drop
/// the rest without a word, so a hand-edited or newer-tool row would load as a
/// record nobody here understood, and every answer computed from it would be
/// quietly wrong.
///
/// The store therefore refuses it at the door, for every family it reads and
/// not only the ideation ones: requirements, rules, sources, messages
/// and the rest all pass [`record_from_line`]. The refusal names the file, the
/// record if the line carries an id, and both versions, because the fix is to
/// find that line and decide what it should say.
///
/// Every write is a read first: `mutate_jsonl_locked` loads the shard, hands
/// the records to the caller, and writes the whole shard back, dropping any
/// unrecognised field on the way out. That path therefore calls this same
/// function, on the same raw JSON, before any record is built - see
/// `crate::jsonl`. A write against a shard holding an unsupported row fails
/// before the mutation runs, and the shard is left byte for byte as it was.
///
/// A record object with no `schema_version` makes no claim about its layout;
/// there is nothing to compare, and its own deserializer says whether the
/// field was required. The ideation landing reader applies the same check to
/// the versioned records inside its unversioned batch envelope. The version is
/// read from the raw JSON rather than from a struct because the struct is what
/// we refuse to build until the version is known.
#[rule("rule_reads_supported_version_only")]
pub fn ensure_supported_record_version(
    path: &Utf8Path,
    line_number: usize,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    let Some((id, version)) = first_unsupported_record(value) else {
        return Ok(());
    };
    let record = id.map_or_else(|| "record".to_string(), |id| format!("record {id}"));
    anyhow::bail!(
        "{path} line {line_number}: {record} has schema_version {version}, \
         but this build reads schema_version {} only",
        SUPPORTED_SCHEMA_VERSION.0
    )
}

fn first_unsupported_record(value: &serde_json::Value) -> Option<(Option<&str>, u64)> {
    let record = value.as_object()?;
    unsupported_object_version(record)
}

fn ensure_supported_nested_record_versions(
    path: &Utf8Path,
    line_number: usize,
    value: &serde_json::Value,
    fields: &[&str],
) -> anyhow::Result<()> {
    let unsupported = fields
        .iter()
        .filter_map(|field| value.get(*field).and_then(serde_json::Value::as_array))
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .find_map(unsupported_object_version);
    let Some((id, version)) = unsupported else {
        return Ok(());
    };
    let record = id.map_or_else(|| "record".to_string(), |id| format!("record {id}"));
    anyhow::bail!(
        "{path} line {line_number}: {record} has schema_version {version}, \
         but this build reads schema_version {} only",
        SUPPORTED_SCHEMA_VERSION.0
    )
}

pub fn ensure_supported_ideation_landing_versions(
    path: &Utf8Path,
    line_number: usize,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    ensure_supported_nested_record_versions(
        path,
        line_number,
        value,
        IDEATION_LANDING_RECORD_FIELDS,
    )
}

fn unsupported_object_version(
    record: &serde_json::Map<String, serde_json::Value>,
) -> Option<(Option<&str>, u64)> {
    let version = record
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)?;
    (version != u64::from(SUPPORTED_SCHEMA_VERSION.0)).then(|| {
        (
            record.get("id").and_then(serde_json::Value::as_str),
            version,
        )
    })
}

fn read_records<T: DeserializeOwned>(
    path: &Utf8Path,
    fields: Fields,
    prepare: fn(&mut serde_json::Value),
    nested_record_fields: &[&str],
) -> anyhow::Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut records = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let mut value: serde_json::Value = serde_json::from_str(line)?;
        prepare(&mut value);
        records.push(record_from_line(
            path,
            index + 1,
            line,
            value,
            fields,
            nested_record_fields,
        )?);
    }
    Ok(records)
}

const fn leave_as_written(_value: &mut serde_json::Value) {}

pub(super) fn read_jsonl<T: DeserializeOwned>(path: &Utf8Path) -> anyhow::Result<Vec<T>> {
    crate::publication::with_state_path_access(path, || read_jsonl_unlocked(path))
}

fn read_jsonl_unlocked<T: DeserializeOwned>(path: &Utf8Path) -> anyhow::Result<Vec<T>> {
    read_records(path, Fields::Open, leave_as_written, NO_NESTED_RECORDS)
}

pub(super) fn read_ideation_landings<T: DeserializeOwned>(
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    crate::publication::with_state_path_access(path, || {
        read_records(
            path,
            Fields::Open,
            leave_as_written,
            IDEATION_LANDING_RECORD_FIELDS,
        )
    })
}

pub(super) fn read_legacy_dispositions(path: &Utf8Path) -> anyhow::Result<Vec<DispositionRecord>> {
    crate::publication::with_state_path_access(path, || read_legacy_dispositions_unlocked(path))
}

fn read_legacy_dispositions_unlocked(path: &Utf8Path) -> anyhow::Result<Vec<DispositionRecord>> {
    read_records(
        path,
        Fields::Open,
        normalize_disposition_aliases,
        NO_NESTED_RECORDS,
    )
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

pub(super) fn read_jsonl_closed<T: DeserializeOwned>(path: &Utf8Path) -> anyhow::Result<Vec<T>> {
    crate::publication::with_state_path_access(path, || read_jsonl_closed_unlocked(path))
}

fn read_jsonl_closed_unlocked<T: DeserializeOwned>(path: &Utf8Path) -> anyhow::Result<Vec<T>> {
    read_records(path, Fields::Closed, leave_as_written, NO_NESTED_RECORDS)
}

pub(super) fn deserialize_closed<T: DeserializeOwned>(input: &str) -> anyhow::Result<T> {
    let mut unknown = None;
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let value = serde_ignored::deserialize(&mut deserializer, |path| {
        if unknown.is_none() {
            unknown = Some(path.to_string());
        }
    })?;
    if let Some(path) = unknown {
        anyhow::bail!("unknown field `{path}`");
    }
    Ok(value)
}

fn read_jsonl_shards<T: DeserializeOwned>(
    shard_paths: Vec<Utf8PathBuf>,
    shard_kind: &str,
) -> anyhow::Result<Vec<T>> {
    let mut records = Vec::new();
    for path in shard_paths {
        crate::test_probes::record_read(&path);
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {shard_kind} shard {}", path.as_str()))?;
        for (index, line) in contents.lines().enumerate() {
            let context = || {
                format!(
                    "failed to parse {shard_kind} shard {} line {}",
                    path.as_str(),
                    index + 1
                )
            };
            let value: serde_json::Value = serde_json::from_str(line).with_context(context)?;
            records.push(
                record_from_line(
                    &path,
                    index + 1,
                    line,
                    value,
                    Fields::Open,
                    NO_NESTED_RECORDS,
                )
                .with_context(context)?,
            );
        }
    }
    Ok(records)
}

pub(super) fn read_message_shards(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Message>> {
    crate::publication::with_repository_publication(layout, || {
        read_jsonl_shards(message_shard_paths(layout, scope)?, "message")
    })
}

/// Every month shard of the scope's messages, sorted. All message reads
/// discover their shards here.
pub fn message_shard_paths(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<Utf8PathBuf>> {
    let threads_dir = shards::threads_path(layout, scope)
        .parent()
        .expect("threads path must have a parent")
        .to_path_buf();
    if !threads_dir.exists() {
        return Ok(Vec::new());
    }
    let mut shard_paths = Vec::new();
    for entry in std::fs::read_dir(&threads_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
                anyhow::anyhow!("non-UTF-8 message shard path: {}", path.display())
            })?;
            if is_message_month_shard(&path) {
                shard_paths.push(path);
            }
        }
    }
    shard_paths.sort();
    Ok(shard_paths)
}

fn is_message_month_shard(path: &Utf8Path) -> bool {
    let Some(file_name) = path.file_name() else {
        return false;
    };
    let bytes = file_name.as_bytes();
    bytes.len() == "2026-07.jsonl".len()
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && &bytes[7..] == b".jsonl"
}

#[cfg(test)]
mod read_guard_tests;
