//! JSON Lines shards: the row code that builds and checks shard bytes, and,
//! with the `host` feature, the file writers of derived state, fixtures and
//! the merge driver.

#[cfg(feature = "host")]
use crate::layout::ProvenanceLayout;
use crate::publication::{protect_rows, WritePermissions};
use crate::state_store::readers::{ensure_no_legacy_retirement, parse_line};
use crate::state_store::StateStore;
use camino::Utf8Path;
#[cfg(feature = "host")]
use fs2::FileExt;
use serde::{de::DeserializeOwned, Serialize};
#[cfg(feature = "host")]
use std::fs::{File, OpenOptions};

mod preservation;

use preservation::{LoadedRecords, RawRecord};

/// An exclusive lock on the lock file at a path. Released on drop.
#[cfg(feature = "host")]
pub(crate) struct AdvisoryLock {
    file: File,
}

#[cfg(feature = "host")]
impl AdvisoryLock {
    pub(crate) fn acquire(path: &Utf8Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                crate::refusal::caused(error, format!("create advisory lock directory {parent}"))
            })?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| crate::refusal::caused(error, format!("open advisory lock {path}")))?;
        file.lock_exclusive().map_err(|error| {
            crate::refusal::caused(error, format!("acquire advisory lock {path}"))
        })?;
        Ok(Self { file })
    }
}

#[cfg(feature = "host")]
impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(feature = "host")]
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

/// Writes a test fixture shard in its own publication section. It carries no
/// write permission, and inside a held section or stage body it refuses.
#[cfg(all(feature = "host", any(test, feature = "test-fixture")))]
pub fn write_jsonl_atomic<T: Serialize>(path: &Utf8Path, records: &[T]) -> anyhow::Result<()> {
    with_state_publication(path, || write_jsonl_atomic_under_publication(path, records))
}

/// Mutates one file of derived state, such as the verification runs, under
/// its advisory lock, and writes the file in place. The caller holds the
/// section. Canonical state publishes through the storage port instead.
#[cfg(feature = "host")]
pub(crate) fn mutate_jsonl_locked<T, R>(
    path: &Utf8Path,
    lock_path: &Utf8Path,
    mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<R>,
) -> anyhow::Result<R>
where
    T: DeserializeOwned + Serialize,
{
    let _lock = AdvisoryLock::acquire(lock_path)?;
    let mut loaded = load_jsonl_file(path)?;
    let result = mutate(loaded.records_mut())?;
    let lines = loaded.into_lines(path)?;
    guard_jsonl_lines(&files_of(path), path, &lines, &WritePermissions::none())?;
    write_jsonl_lines_atomic_unlocked(path, &lines)
        .map_err(crate::write_error::publication_started)?;
    Ok(result)
}

/// Reads the records of one file of derived state, such as the verification
/// runs, under its advisory lock. A file that does not exist has no records.
#[cfg(feature = "host")]
pub(crate) fn read_jsonl_locked<T>(path: &Utf8Path, lock_path: &Utf8Path) -> anyhow::Result<Vec<T>>
where
    T: DeserializeOwned + Serialize,
{
    let _lock = AdvisoryLock::acquire(lock_path)?;
    let mut loaded = load_jsonl_file(path)?;
    Ok(std::mem::take(loaded.records_mut()))
}

/// Loads the file at `path`, or no records when the file does not exist.
#[cfg(feature = "host")]
fn load_jsonl_file<T>(path: &Utf8Path) -> anyhow::Result<LoadedRecords<T>>
where
    T: DeserializeOwned + Serialize,
{
    let contents = path
        .exists()
        .then(|| std::fs::read_to_string(path))
        .transpose()?;
    load_jsonl(path, contents.as_deref())
}

/// Loads the shard at `path` from `contents`, runs `mutate`, and returns the
/// new bytes of the shard when `mutate` returns `true`. A change to an
/// enrolled row needs a grant in the permissions of `store`, which holds the
/// section. The caller writes the bytes.
pub(crate) fn mutate_jsonl_bytes<T>(
    store: &StateStore,
    path: &Utf8Path,
    contents: Option<&str>,
    mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<bool>,
) -> anyhow::Result<Option<Vec<u8>>>
where
    T: DeserializeOwned + Serialize,
{
    let ((), bytes) = mutate_jsonl_bytes_with(store, path, contents, |records| {
        mutate(records).map(|changed| ((), changed))
    })?;
    Ok(bytes)
}

/// Like [`mutate_jsonl_bytes`], but `mutate` also gives a value that the
/// caller receives with the bytes.
pub(crate) fn mutate_jsonl_bytes_with<T, R>(
    store: &StateStore,
    path: &Utf8Path,
    contents: Option<&str>,
    mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<(R, bool)>,
) -> anyhow::Result<(R, Option<Vec<u8>>)>
where
    T: DeserializeOwned + Serialize,
{
    let mut loaded = load_jsonl(path, contents)?;
    let (result, changed) = mutate(loaded.records_mut())?;
    if !changed {
        return Ok((result, None));
    }
    let lines = loaded.into_lines(path)?;
    guard_jsonl_lines(store, path, &lines, store.permissions())?;
    Ok((result, Some(jsonl_bytes(&lines))))
}

/// The read a write is built on, guarded the same way an ordinary read is.
///
/// A mutation rewrites the whole shard. Each raw line passes
/// [`ensure_no_legacy_retirement`](crate::state_store::readers::ensure_no_legacy_retirement),
/// the same check the read choke point makes, before any record is built.
///
/// The raw line stays with its typed record. An unchanged record keeps its
/// raw line when the shard is saved, so the stored JSON line content that
/// this build does not own survives unchanged. Line terminators are normalized
/// to `\n`. A changed record carries the
/// row's top-level unknown members onto its new line, and the write is
/// refused before anything is published when a changed row holds stored data
/// that cannot be carried over safely.
fn load_jsonl<T>(path: &Utf8Path, contents: Option<&str>) -> anyhow::Result<LoadedRecords<T>>
where
    T: DeserializeOwned + Serialize,
{
    let Some(contents) = contents else {
        return Ok(LoadedRecords::default());
    };
    let mut loaded = LoadedRecords::default();
    for (index, line) in contents.lines().enumerate() {
        let value = parse_line(path, index + 1, line)?;
        ensure_no_legacy_retirement(path, index + 1, &value)?;
        let (record, raw) = RawRecord::deserialize(path, line, index + 1)?;
        loaded.push(record, raw);
    }
    Ok(loaded)
}

/// Writes one JSONL shard while the caller holds the publication lock. It
/// carries no write permission, so it cannot change an enrolled row.
#[cfg(all(feature = "host", any(test, feature = "test-fixture")))]
pub(crate) fn write_jsonl_atomic_under_publication<T: Serialize>(
    path: &Utf8Path,
    records: &[T],
) -> anyhow::Result<()> {
    protect_rows(&files_of(path), path, records, &WritePermissions::none())?;
    let lines = records
        .iter()
        .map(to_stable_json)
        .collect::<anyhow::Result<Vec<_>>>()?;
    write_jsonl_lines_atomic_unlocked(path, &lines)
}

/// Writes merged records from the opaque rows read from the merge inputs.
///
/// The writer selects one stored line that has the exact value of each merged
/// record. Callers cannot provide raw lines independently. A mismatch is
/// refused before the target is replaced. JSON line bytes are preserved; line
/// terminators are written as `\n`. The write takes its own publication
/// section, so inside a held section or stage body it refuses.
#[cfg(feature = "host")]
pub fn write_preserved_jsonl_atomic(
    path: &Utf8Path,
    ours: &[crate::merge::StoredRow],
    theirs: &[crate::merge::StoredRow],
    records: &[crate::merge::CanonicalRecord],
) -> anyhow::Result<()> {
    let lines = crate::merge::preserved_lines(ours, theirs, records)?;
    with_state_publication(path, || {
        guard_jsonl_lines(&files_of(path), path, &lines, &WritePermissions::none())?;
        write_jsonl_lines_atomic_unlocked(path, &lines)
    })
}

/// A store that reads the files of the repository of `path` with no lock,
/// for a raw writer that holds the section and writes the files itself.
#[cfg(feature = "host")]
fn files_of(path: &Utf8Path) -> StateStore {
    StateStore::read_only(ProvenanceLayout::new(path.to_owned()))
}

fn guard_jsonl_lines(
    store: &StateStore,
    path: &Utf8Path,
    lines: &[String],
    permissions: &WritePermissions,
) -> anyhow::Result<()> {
    let records = lines
        .iter()
        .map(|line| serde_json::from_str::<serde_json::Value>(line))
        .collect::<Result<Vec<_>, _>>()?;
    protect_rows(store, path, &records, permissions)
}

#[cfg(feature = "host")]
fn write_jsonl_lines_atomic_unlocked(path: &Utf8Path, lines: &[String]) -> anyhow::Result<()> {
    crate::storage::local::durability::write_synced_file(path, &jsonl_bytes(lines))
}

/// The lines, each with a final `\n`.
fn jsonl_bytes(lines: &[String]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(lines.iter().map(|line| line.len() + 1).sum());
    for line in lines {
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

/// Runs a raw write in the publication section of the repository that holds
/// `path`. The raw writers carry no permission and get no held store.
#[cfg(feature = "host")]
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
        crate::publication::with_repository_publication(&layout, |_| run())
    } else {
        run()
    }
}

#[cfg(test)]
#[cfg(feature = "host")]
mod tests;
