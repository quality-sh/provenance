//! The scanned source files of the two revisions of one commit range.
//!
//! A scan depends on the file path and the blob content only, so the scan
//! of a path and blob id is read and parsed once. The base and head of a
//! range share each file that the range did not change. The process keeps
//! the two revisions of the last range, so a host that answers the same
//! range again starts no blob read and parses no file. Only one range is
//! kept, so the memory stays at the size of one answer.

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
}

static KEPT: Mutex<Vec<Arc<Revision>>> = Mutex::new(Vec::new());

/// Below this count, one thread parses all files.
const FILES_PER_THREAD: usize = 64;

pub struct RangeScans {
    pub base: Arc<Revision>,
    pub head: Arc<Revision>,
}

/// The scanned source files of `base` and of `head`. The git reads run in
/// the same order as reading the two revisions one after the other, so
/// the same failure wins.
pub fn scan_range(repo: &Utf8Path, base: &str, head: &str) -> anyhow::Result<RangeScans> {
    let kept = lock().clone();
    let base = scan_revision(repo, base, &kept)?;
    let mut known = vec![Arc::clone(&base)];
    known.extend(kept);
    let head = scan_revision(repo, head, &known)?;
    let mut range = vec![Arc::clone(&base)];
    if !Arc::ptr_eq(&base, &head) {
        range.push(Arc::clone(&head));
    }
    *lock() = range;
    Ok(RangeScans { base, head })
}

/// A known revision with the same tree is used as it is. Otherwise each
/// file that a known revision holds at the same path and blob is copied,
/// and only the other blobs are read and parsed.
fn scan_revision(
    repo: &Utf8Path,
    revision: &str,
    known: &[Arc<Revision>],
) -> anyhow::Result<Arc<Revision>> {
    let blobs = git::revision_source_blobs(repo, revision)?;
    if let Some(same) = known.iter().find(|known| known.blobs == blobs) {
        return Ok(Arc::clone(same));
    }
    let mut scans = HashMap::new();
    for known in known {
        for (blob, file) in known.blobs.iter().zip(&known.files) {
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
        .collect();
    Ok(Arc::new(Revision { blobs, files }))
}

/// Parses the files on a few threads and keeps the input order.
fn parse(files: Vec<RevisionFile>) -> Vec<FileScanWithContent> {
    in_order::map(files, FILES_PER_THREAD, gate::scan_file)
}

fn lock() -> MutexGuard<'static, Vec<Arc<Revision>>> {
    KEPT.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
