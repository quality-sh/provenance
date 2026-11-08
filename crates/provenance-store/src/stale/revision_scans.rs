//! The scanned source files of the two revisions of one commit range.
//!
//! A scan depends on the path and the blob only, so a range reads and
//! parses each path and blob once. An opened repository keeps the two
//! revisions of the last range it answered, so a repeated range reads no
//! blob and parses no file. docs/cache.md gives the limits of what is kept.

use super::git::{self, RevisionFile};
use super::{gate, in_order};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_scanner::FileScanWithContent;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The source blobs of one revision tree, in tree order, and their scans
/// in the same order.
pub struct Revision {
    /// The blob id and the repository path of each source file.
    blobs: Vec<(String, Utf8PathBuf)>,
    pub files: Vec<FileScanWithContent>,
    /// The bytes of source text that the scans hold.
    bytes: usize,
}

/// The revisions of the last range that one opened repository answered.
///
/// The repository handle owns it, so it lives as long as the open and
/// each clone of the handle shares it.
pub struct KeptScans {
    revisions: Mutex<Vec<Arc<Revision>>>,
    max_bytes: usize,
}

/// A repository keeps at most this much source text.
const KEPT_BYTES: usize = 256 * 1024 * 1024;

/// Each parse thread takes at least this many files.
const FILES_PER_THREAD: usize = 64;

pub struct RangeScans {
    pub base: Arc<Revision>,
    pub head: Arc<Revision>,
}

/// The scanned source files of `base` and of `head`. The git reads run in
/// the same order as reading the two revisions one after the other, so
/// the same failure wins.
pub fn scan_range(
    kept: &KeptScans,
    repo: &Utf8Path,
    base: &str,
    head: &str,
) -> anyhow::Result<RangeScans> {
    let known = kept.lock().clone();
    let base = scan_revision(repo, base, None, &known)?;
    let head = scan_revision(repo, head, Some(&base), &known)?;
    kept.keep(&base, &head);
    Ok(RangeScans { base, head })
}

/// A revision with the same tree is used as it is. Otherwise each file
/// that a revision holds at the same path and blob is copied, and only
/// the other blobs are read and parsed.
///
/// `earlier` is the base of this range. A process with no kept revisions
/// reads each blob of the revision that `earlier` does not hold. The kept
/// revisions replace those reads only when each of those blobs is still
/// in the object store; otherwise the same reads run, so the same failure
/// wins.
fn scan_revision(
    repo: &Utf8Path,
    revision: &str,
    earlier: Option<&Arc<Revision>>,
    kept: &[Arc<Revision>],
) -> anyhow::Result<Arc<Revision>> {
    let blobs = git::revision_source_blobs(repo, revision)?;
    if let Some(same) = earlier.filter(|earlier| earlier.blobs == blobs) {
        return Ok(Arc::clone(same));
    }
    let mut scans = scans_by_blob(earlier.into_iter());
    let kept_scans = scans_by_blob(kept.iter());
    let reused = blobs
        .iter()
        .filter(|blob| !scans.contains_key(blob) && kept_scans.contains_key(blob))
        .map(|(object, _)| object.as_str())
        .collect::<Vec<_>>();
    if reused.is_empty() || git::blobs_exist(repo, &reused) {
        if let Some(same) = kept.iter().find(|kept| kept.blobs == blobs) {
            return Ok(Arc::clone(same));
        }
        for (blob, file) in kept_scans {
            scans.entry(blob).or_insert(file);
        }
    }
    let missing = blobs
        .iter()
        .filter(|blob| !scans.contains_key(blob))
        .cloned()
        .collect::<Vec<_>>();
    let mut parsed = parse(git::read_blobs(repo, missing)?).into_iter();
    let files = blobs
        .iter()
        .map(|blob| {
            scans.get(blob).map_or_else(
                || parsed.next().expect("one parsed file for each read blob"),
                |&file| file.clone(),
            )
        })
        .collect::<Vec<FileScanWithContent>>();
    let bytes = files.iter().map(|file| file.content.len()).sum();
    Ok(Arc::new(Revision {
        blobs,
        files,
        bytes,
    }))
}

fn scans_by_blob<'r>(
    revisions: impl Iterator<Item = &'r Arc<Revision>>,
) -> HashMap<&'r (String, Utf8PathBuf), &'r FileScanWithContent> {
    let mut scans = HashMap::new();
    for revision in revisions {
        for (blob, file) in revision.blobs.iter().zip(&revision.files) {
            scans.entry(blob).or_insert(file);
        }
    }
    scans
}

/// Parses the files on a few threads and keeps the input order.
fn parse(files: Vec<RevisionFile>) -> Vec<FileScanWithContent> {
    in_order::map(files, FILES_PER_THREAD, gate::scan_file)
}

impl KeptScans {
    pub(crate) const fn new() -> Self {
        Self::with_limit(KEPT_BYTES)
    }

    pub(super) const fn with_limit(max_bytes: usize) -> Self {
        Self {
            revisions: Mutex::new(Vec::new()),
            max_bytes,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Arc<Revision>>> {
        self.revisions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps the range in place of the last range. A range larger than the
    /// byte limit is not kept, and the last range is dropped.
    fn keep(&self, base: &Arc<Revision>, head: &Arc<Revision>) {
        let mut range = vec![Arc::clone(base)];
        if !Arc::ptr_eq(base, head) {
            range.push(Arc::clone(head));
        }
        if range.iter().map(|revision| revision.bytes).sum::<usize>() > self.max_bytes {
            range.clear();
        }
        *self.lock() = range;
    }
}

impl std::fmt::Debug for KeptScans {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("KeptScans").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
