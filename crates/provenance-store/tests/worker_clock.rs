//! The store reads the time through the time crate, which reaches the clock
//! through JavaScript on wasm32. The standard library clock panics in a
//! Worker, so only the local adapter, which never runs there, may name it.

use provenance_macros::verifies;
use std::fs;
use std::path::{Path, PathBuf};

const STD_CLOCK: &str = "SystemTime";

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// The local adapter: `storage/local.rs` and everything under `storage/local/`.
fn is_local_adapter(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap();
    let local = Path::new("storage").join("local");
    relative.starts_with(&local) || relative == local.with_extension("rs")
}

fn names_std_clock(source: &str) -> bool {
    source
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|word| word == STD_CLOCK)
}

#[test]
#[verifies("rule_store_time_works_in_a_worker", examples)]
fn store_code_outside_the_local_adapter_names_no_std_clock() {
    let root = src();
    let findings: Vec<String> = rust_files(&root)
        .into_iter()
        .filter(|path| !is_local_adapter(path, &root))
        .filter(|path| names_std_clock(&fs::read_to_string(path).unwrap()))
        .map(|path| path.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    assert!(
        findings.is_empty(),
        "these files name {STD_CLOCK}; read the time with time::OffsetDateTime::now_utc: {findings:?}"
    );
}

#[test]
fn the_scan_finds_the_std_clock_and_skips_the_local_adapter() {
    assert!(names_std_clock("let now = std::time::SystemTime::now();"));
    assert!(!names_std_clock("let now = MySystemTimeLike::now();"));
    let root = src();
    assert!(is_local_adapter(
        &root.join("storage/local/whole_set.rs"),
        &root
    ));
    assert!(is_local_adapter(&root.join("storage/local.rs"), &root));
    assert!(!is_local_adapter(&root.join("storage/memory.rs"), &root));
}
