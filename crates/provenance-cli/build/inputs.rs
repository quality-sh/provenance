//! The tracked paths that enter the CLI build. The build script runs again when
//! one of them changes, and it marks the build `-dirty` when git reports a
//! change in one of them. The version test reads the same list.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Paths outside `crates` that the build reads: the workspace manifest and lock
/// file, the Cargo configuration, the bundled skill sources, and the pin of the
/// review page bundle that CI embeds.
const ROOT_INPUTS: [&str; 5] = [
    "Cargo.toml",
    "Cargo.lock",
    ".cargo",
    "skills",
    "tools/review-assets.json",
];

/// Crate directories that hold test, bench, and example targets. Their content
/// does not enter the CLI binary, so a change there does not run the script.
const NOT_IN_BINARY: [&str; 3] = ["tests", "benches", "examples"];

/// The paths relative to the workspace root, with `/` between components, in a
/// fixed order. Each workspace crate adds each of its top-level entries that
/// holds a tracked file, except the test, bench, and example directories.
/// `git` runs git in `root` with the given arguments. Untracked entries, for
/// example generated package copies, are not on the list, so a generator that
/// writes them does not run the script again. When git cannot list the tracked
/// files, each crate adds all of its top-level entries on disk.
pub fn paths(root: &Path, git: impl Fn(&[&str]) -> Option<String>) -> Vec<String> {
    let mut paths: Vec<String> = ROOT_INPUTS.iter().map(|&path| path.to_owned()).collect();
    let entries = match git(&["ls-files", "-z", "--", "crates"]) {
        Some(tracked) => tracked_entries(&tracked),
        None => entries_on_disk(root),
    };
    paths.extend(
        entries
            .into_iter()
            .filter(|(_, name)| !NOT_IN_BINARY.contains(&name.as_str()))
            .map(|(krate, name)| format!("crates/{krate}/{name}")),
    );
    paths.retain(|path| absolute(root, path).exists());
    paths
}

/// The crate and top-level entry names of the `crates/<crate>/<entry>[/...]`
/// file paths that `git ls-files -z` gives, sorted and without duplicates.
fn tracked_entries(listing: &str) -> BTreeSet<(String, String)> {
    listing
        .split('\0')
        .filter_map(|file| {
            let (krate, rest) = file.strip_prefix("crates/")?.split_once('/')?;
            let name = rest.split('/').next()?;
            Some((krate.to_owned(), name.to_owned()))
        })
        .collect()
}

fn entries_on_disk(root: &Path) -> BTreeSet<(String, String)> {
    let mut entries = BTreeSet::new();
    for krate in sorted_entries(&root.join("crates")) {
        if !krate.is_dir() {
            continue;
        }
        let Some(krate_name) = file_name(&krate) else {
            continue;
        };
        for entry in sorted_entries(&krate) {
            if let Some(name) = file_name(&entry) {
                entries.insert((krate_name.clone(), name));
            }
        }
    }
    entries
}

/// The path on disk for one entry of [`paths`].
pub fn absolute(root: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

fn sorted_entries(directory: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.path()))
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    entries
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name()?.to_str().map(str::to_owned)
}
