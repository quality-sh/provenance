//! The scanned source files of the two revisions of one commit range.
//!
//! A scan depends on the path and the blob only, so a range reads and
//! parses each path and blob once. The process keeps the two revisions of
//! the last range of each repository, so a repeated range reads no blob
//! and parses no file. docs/cache.md gives the limits of what is kept.

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

/// The revisions of the last range of each repository, the least recently
/// used repository first.
pub struct KeptScans {
    repositories: Vec<(Utf8PathBuf, Vec<Arc<Revision>>)>,
    max_repositories: usize,
    max_bytes: usize,
}

/// The process keeps the ranges of at most this many repositories.
const KEPT_REPOSITORIES: usize = 4;
/// The process keeps at most this much source text over all repositories.
const KEPT_BYTES: usize = 256 * 1024 * 1024;

static KEPT: Mutex<KeptScans> = Mutex::new(KeptScans::new(KEPT_REPOSITORIES, KEPT_BYTES));

/// Each parse thread takes at least this many files.
const FILES_PER_THREAD: usize = 64;

pub struct RangeScans {
    pub base: Arc<Revision>,
    pub head: Arc<Revision>,
}

/// The scanned source files of `base` and of `head`. The git reads run in
/// the same order as reading the two revisions one after the other, so
/// the same failure wins.
pub fn scan_range(repo: &Utf8Path, base: &str, head: &str) -> anyhow::Result<RangeScans> {
    scan_range_with(&KEPT, repo, base, head)
}

pub(super) fn scan_range_with(
    kept: &Mutex<KeptScans>,
    repo: &Utf8Path,
    base: &str,
    head: &str,
) -> anyhow::Result<RangeScans> {
    let known = lock(kept).revisions(repo);
    let base = scan_revision(repo, base, None, &known)?;
    let head = scan_revision(repo, head, Some(&base), &known)?;
    lock(kept).keep(repo, &base, &head);
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
    let t = std::time::Instant::now();
    let blobs = git::revision_source_blobs(repo, revision)?;
    crate::prof(&format!("      ls-tree {revision:.8} ({} blobs)", blobs.len()), t);
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
    let t = std::time::Instant::now();
    let usable = reused.is_empty() || git::blobs_exist(repo, &reused);
    crate::prof(&format!("      cat-file --batch-check ({} blobs)", reused.len()), t);
    if usable {
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
    let count = missing.len();
    let t = std::time::Instant::now();
    let read = git::read_blobs(repo, missing)?;
    crate::prof(&format!("      cat-file --batch ({count} blobs)"), t);
    let t = std::time::Instant::now();
    let mut parsed = parse(read).into_iter();
    crate::prof(&format!("      parse ({count} files)"), t);
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
    pub(super) const fn new(max_repositories: usize, max_bytes: usize) -> Self {
        Self {
            repositories: Vec::new(),
            max_repositories,
            max_bytes,
        }
    }

    fn revisions(&self, repo: &Utf8Path) -> Vec<Arc<Revision>> {
        self.repositories
            .iter()
            .find(|(kept, _)| kept == repo)
            .map(|(_, revisions)| revisions.clone())
            .unwrap_or_default()
    }

    /// Keeps the range as the last range of `repo`, then drops the least
    /// recently used repositories until the limits hold. A range larger
    /// than the byte limit is not kept.
    fn keep(&mut self, repo: &Utf8Path, base: &Arc<Revision>, head: &Arc<Revision>) {
        let mut range = vec![Arc::clone(base)];
        if !Arc::ptr_eq(base, head) {
            range.push(Arc::clone(head));
        }
        self.repositories.retain(|(kept, _)| kept != repo);
        self.repositories.push((repo.to_path_buf(), range));
        while self.repositories.len() > self.max_repositories || self.bytes() > self.max_bytes {
            self.repositories.remove(0);
        }
    }

    fn bytes(&self) -> usize {
        self.repositories
            .iter()
            .flat_map(|(_, revisions)| revisions)
            .map(|revision| revision.bytes)
            .sum()
    }
}

fn lock(kept: &Mutex<KeptScans>) -> MutexGuard<'_, KeptScans> {
    kept.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
