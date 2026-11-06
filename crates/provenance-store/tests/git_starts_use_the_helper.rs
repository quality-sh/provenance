//! Implementation aid: no Rule states where Git starts. Each Git start goes through
//! `provenance_store::git_process::git`, so a Windows process without a console opens no
//! window for Git (bead provenance-a5ru). This test refuses a direct Git start in any Rust
//! file of the workspace.

use std::path::{Path, PathBuf};

const HELPER: &str = "crates/provenance-store/src/git_process.rs";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type().unwrap().is_dir() {
            if !name.starts_with('.') && !matches!(name.as_ref(), "target" | "node_modules") {
                rust_files(&path, files);
            }
        } else if name.ends_with(".rs") {
            files.push(path);
        }
    }
}

/// The direct Git starts in `source`, with the spaces removed so that a line break or a
/// space inside the call does not hide it.
fn direct_git_starts(source: &str) -> usize {
    let compact: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    // The patterns are built so that this file does not match itself.
    ["\"git\")", "\"git.exe\")"]
        .iter()
        .map(|program| compact.matches(&format!("Command::new({program}")).count())
        .sum()
}

#[test]
fn direct_git_starts_are_found_with_any_spacing() {
    let call = ["Command::new", "(\n    \"git\"\n)"].concat();
    assert_eq!(direct_git_starts(&call), 1);
    let call = ["ProcessCommand::new", "(\"git.exe\")"].concat();
    assert_eq!(direct_git_starts(&call), 1);
    assert_eq!(direct_git_starts("Command::new(\"cargo\")"), 0);
}

#[test]
fn only_the_git_process_helper_starts_git() {
    let root = workspace_root();
    let mut files = Vec::new();
    for directory in ["crates", "examples", "tools", "fixtures"] {
        if root.join(directory).is_dir() {
            rust_files(&root.join(directory), &mut files);
        }
    }
    assert!(files.iter().any(|file| file.ends_with(HELPER)));
    let offenders: Vec<String> = files
        .iter()
        .filter(|file| !file.ends_with(HELPER))
        .filter(|file| direct_git_starts(&std::fs::read_to_string(file).unwrap()) > 0)
        .map(|file| file.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "start Git with provenance_store::git_process::git() in: {offenders:?}"
    );
}
