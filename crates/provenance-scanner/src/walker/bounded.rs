//! A tree scan that stops at a file count.
//!
//! The whole-tree walk sorts after it reads, so a cut taken in walk order
//! would fall on different files from one clone to the next. This walk
//! visits siblings by file name, counts only the files a language reads,
//! and keeps the first `max_files` of them in that order; the answer is
//! sorted by path, so under the limit it is what `scan_path` gives.

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;

use super::{is_ignored_directory, scan_file, FileScan, Language};

/// Scans at most `max_files` language files under `path`, the first that
/// many in sorted walk order, and says whether the walk stopped short.
/// `true` means the scanned sites are a lower bound.
#[rule("rule_scan_cut_is_deterministic_and_reported")]
pub fn scan_path_bounded(
    path: &Utf8Path,
    max_files: usize,
) -> anyhow::Result<(Vec<FileScan>, bool)> {
    let mut scans = Vec::new();
    let mut cut = false;
    for entry in walkdir::WalkDir::new(path)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_ignored_directory(entry))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(file_path) = Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).ok() else {
            continue;
        };
        let Some(language) = file_path.extension().and_then(Language::from_extension) else {
            continue;
        };
        if scans.len() == max_files {
            cut = true;
            break;
        }
        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("read source file {file_path}"))?;
        scans.push(scan_file(&file_path, language, &content));
    }
    scans.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    Ok((scans, cut))
}
