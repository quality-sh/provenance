use crate::state_store::readers::ensure_supported_record_version;
use camino::Utf8Path;
use fs2::FileExt;
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;

mod preservation;

use preservation::{LoadedRecords, RawRecord};

struct AdvisoryLock {
    file: File,
}

impl AdvisoryLock {
    fn acquire(path: &Utf8Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| anyhow::anyhow!("open advisory lock {path}: {error}"))?;
        file.lock_exclusive()
            .map_err(|error| anyhow::anyhow!("acquire advisory lock {path}: {error}"))?;
        Ok(Self { file })
    }
}

impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn with_advisory_lock<R>(
    path: &Utf8Path,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let _lock = AdvisoryLock::acquire(path)?;
    operation()
}

pub fn to_stable_json<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string(value)?)
}

pub fn write_jsonl_atomic<T: Serialize>(path: &Utf8Path, records: &[T]) -> anyhow::Result<()> {
    with_state_publication(path, || write_jsonl_atomic_under_publication(path, records))
}

pub fn mutate_jsonl_locked<T, R>(
    path: &Utf8Path,
    lock_path: &Utf8Path,
    mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<R>,
) -> anyhow::Result<R>
where
    T: DeserializeOwned + Serialize,
{
    with_state_publication(path, || {
        let _lock = AdvisoryLock::acquire(lock_path)?;
        let mut loaded = read_jsonl_unlocked(path)?;
        let result = mutate(loaded.records_mut())?;
        crate::review::guard::protect_rows(path, loaded.records())?;
        let lines = loaded.into_lines(path)?;
        write_jsonl_lines_atomic_unlocked(path, &lines)
            .map_err(crate::write_error::publication_started)?;
        Ok(result)
    })
}

/// The read a write is built on, guarded the same way an ordinary read is.
///
/// A mutation rewrites the whole shard. The version is read from the raw JSON
/// and judged by
/// [`ensure_supported_record_version`](crate::state_store::readers::ensure_supported_record_version),
/// the same function the read choke point calls, before any record is built.
///
/// The raw line stays with its typed record. An unchanged record keeps its
/// raw line when the shard is saved, so stored content this build does not
/// own survives outside rows byte for byte. A changed record carries the
/// row's top-level unknown members onto its new line, and the write is
/// refused before anything is published when a changed row holds stored data
/// that cannot be carried over safely.
fn read_jsonl_unlocked<T>(path: &Utf8Path) -> anyhow::Result<LoadedRecords<T>>
where
    T: DeserializeOwned + Serialize,
{
    if !path.exists() {
        return Ok(LoadedRecords::default());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut loaded = LoadedRecords::default();
    for (index, line) in contents.lines().enumerate() {
        let value: serde_json::Value = serde_json::from_str(line)?;
        ensure_supported_record_version(path, index + 1, &value)?;
        let (record, raw) = RawRecord::deserialize(line, index + 1)?;
        loaded.push(record, raw);
    }
    Ok(loaded)
}

/// Writes one JSONL shard while the caller holds the publication lock.
pub(crate) fn write_jsonl_atomic_under_publication<T: Serialize>(
    path: &Utf8Path,
    records: &[T],
) -> anyhow::Result<()> {
    crate::review::guard::protect_rows(path, records)?;
    write_jsonl_atomic_unlocked(path, records)
}

fn write_jsonl_atomic_unlocked<T: Serialize>(path: &Utf8Path, records: &[T]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    for record in records {
        writeln!(temp, "{}", to_stable_json(record)?)?;
    }
    temp.persist(path)?;
    Ok(())
}

/// Writes stored lines for records the caller already holds verbatim.
///
/// Each line lands exactly as given, so records a merge carried over keep
/// their stored bytes. The records are still walked through the same guards
/// a canonical write faces, and the write refuses before the replacement
/// when a guard objects.
pub fn write_jsonl_lines_atomic<T: Serialize>(
    path: &Utf8Path,
    records: &[T],
    lines: &[String],
) -> anyhow::Result<()> {
    with_state_publication(path, || {
        crate::review::guard::protect_rows(path, records)?;
        write_jsonl_lines_atomic_unlocked(path, lines)
    })
}

fn write_jsonl_lines_atomic_unlocked(path: &Utf8Path, lines: &[String]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    for line in lines {
        writeln!(temp, "{line}")?;
    }
    temp.persist(path)?;
    Ok(())
}

fn with_state_publication<R>(
    path: &Utf8Path,
    run: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    if let Some(state) = path.ancestors().find(|p| {
        p.file_name() == Some("state")
            && p.parent().and_then(Utf8Path::file_name) == Some(".provenance")
    }) {
        let layout = crate::layout::ProvenanceLayout::new(
            state.parent().and_then(Utf8Path::parent).unwrap(),
        );
        crate::publication::with_repository_publication(&layout, run)
    } else {
        run()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    #[derive(Serialize)]
    struct Record {
        id: &'static str,
    }

    #[derive(Deserialize, Serialize)]
    struct NestedRecord {
        schema_version: u32,
        id: String,
        detail: Detail,
    }

    #[derive(Deserialize, Serialize)]
    struct Detail {
        known: String,
    }
    #[test]
    fn writes_newline_terminated_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::from_path_buf(dir.path().join("records.jsonl")).unwrap();
        write_jsonl_atomic(&path, &[Record { id: "one" }]).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{\"id\":\"one\"}\n");
    }

    #[test]
    fn persistence_failure_after_mutation_reports_a_write_failure() {
        use crate::write_error::{PublicationStarted, WriteError, WriteFailure};
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        let mut mutation_completed = false;
        let error = mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |records: &mut Vec<serde_json::Value>| {
                records.push(serde_json::json!({"schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION.0,"id":"one"}));
                // A directory at the destination makes the real atomic replacement fail.
                std::fs::create_dir(&path)?;
                mutation_completed = true;
                Ok(())
            },
        ).unwrap_err();
        assert!(mutation_completed);
        let publication = error.downcast_ref::<PublicationStarted>().unwrap();
        assert!(publication
            .0
            .downcast_ref::<tempfile::PersistError>()
            .is_some());
        assert!(matches!(
            WriteError(error).safe(),
            WriteFailure::WriteFailed
        ));
    }

    #[test]
    fn mutation_refusal_keeps_its_type_and_does_not_publish_in_memory_changes() {
        use crate::write_error::{PublicationStarted, SourceFailure, WriteError, WriteFailure};
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        write_jsonl_atomic(&path, &[serde_json::json!({"schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION.0,"id":"one"})]).unwrap();
        let before = std::fs::read(&path).unwrap();
        let error = mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |records: &mut Vec<serde_json::Value>| -> anyhow::Result<()> {
                records.clear();
                Err(SourceFailure::wrap(
                    WriteFailure::InvalidCompletion,
                    anyhow::anyhow!("invalid completion fixture"),
                ))
            },
        )
        .unwrap_err();
        assert!(error.downcast_ref::<PublicationStarted>().is_none());
        assert_eq!(format!("{error:#}"), "invalid completion fixture");
        assert!(matches!(
            WriteError(error).safe(),
            WriteFailure::InvalidCompletion
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn unrelated_mutation_keeps_a_nested_unknown_row_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        let existing =
            "  {\"id\":\"one\",\"detail\":{\"extension\":true,\"known\":\"value\"},\"schema_version\":2}  ";
        std::fs::write(&path, format!("{existing}\n")).unwrap();

        mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |records: &mut Vec<NestedRecord>| {
                records.push(NestedRecord {
                    schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION.0,
                    id: "two".into(),
                    detail: Detail {
                        known: "new".into(),
                    },
                });
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            format!(
                "{existing}\n{{\"schema_version\":2,\"id\":\"two\",\"detail\":{{\"known\":\"new\"}}}}\n"
            )
        );
    }

    #[test]
    fn mutation_keeps_a_changed_row_with_top_level_unknown_data() {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        std::fs::write(
            &path,
            "{\"schema_version\":2,\"id\":\"one\",\"detail\":{\"known\":\"value\"},\"extension\":true}\n",
        )
        .unwrap();

        mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |records: &mut Vec<NestedRecord>| {
                records[0].detail.known = "changed".into();
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "{\"schema_version\":2,\"id\":\"one\",\"detail\":{\"known\":\"changed\"},\"extension\":true}\n"
        );
    }

    #[test]
    fn mutation_refuses_a_changed_row_with_nested_unknown_data() {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        std::fs::write(
            &path,
            "{\"schema_version\":2,\"id\":\"one\",\"detail\":{\"known\":\"value\",\"extension\":true}}\n",
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let message = mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |records: &mut Vec<NestedRecord>| {
                records[0].detail.known = "changed".into();
                Ok(())
            },
        )
        .unwrap_err()
        .to_string();

        assert!(message.contains("nested unknown field"), "{message}");
        assert!(message.contains("detail.extension"), "{message}");
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn mutation_refuses_a_changed_row_with_a_repeated_unknown_field() {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        std::fs::write(
            &path,
            "{\"schema_version\":2,\"id\":\"one\",\"detail\":{\"known\":\"value\"},\"extension\":1,\"extension\":2}\n",
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();

        let message = mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |records: &mut Vec<NestedRecord>| {
                records[0].detail.known = "changed".into();
                Ok(())
            },
        )
        .unwrap_err()
        .to_string();

        assert!(message.contains("repeated top-level member"), "{message}");
        assert!(message.contains("extension"), "{message}");
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn unsupported_version_refusal_precedes_mutation_and_keeps_the_shard() {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("records.jsonl");
        let future = provenance_core::SUPPORTED_SCHEMA_VERSION.0 + 1;
        std::fs::write(
            &path,
            format!("{{\"schema_version\":{future},\"id\":\"one\"}}\n"),
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut mutation_ran = false;

        let error = mutate_jsonl_locked(
            &path,
            &root.join("records.lock"),
            |_: &mut Vec<NestedRecord>| {
                mutation_ran = true;
                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("has schema_version"));
        assert!(!mutation_ran);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
