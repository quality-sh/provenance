//! Throwaway probe for provenance-2a2z. Not for merge. It measures how a
//! replace of a file fails while another handle has the target open, for
//! `tempfile` persist and for `std::fs::rename`. On Windows the test fails on
//! purpose so that the CI log shows the counts.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const ROUNDS: usize = 400;

fn temp_in(dir: &Path) -> tempfile::NamedTempFile {
    let mut temp = tempfile::NamedTempFile::new_in(dir).unwrap();
    temp.write_all(b"{\"phase\":\"x\"}").unwrap();
    temp.as_file().sync_all().unwrap();
    temp
}

fn persist(dir: &Path, target: &Path) -> Result<(), String> {
    temp_in(dir)
        .persist(target)
        .map(|_| ())
        .map_err(|error| format!("{:?}", error.error.raw_os_error()))
}

fn std_rename(dir: &Path, target: &Path) -> Result<(), String> {
    let mut path = temp_in(dir).into_temp_path();
    let result = std::fs::rename(&path, target);
    if result.is_ok() {
        path.disable_cleanup(true);
    }
    result.map_err(|error| format!("{:?}", error.raw_os_error()))
}

/// Replaces `target` while another handle holds it open the whole time.
fn with_held_handle(replace: fn(&Path, &Path) -> Result<(), String>) -> String {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("marker.json");
    std::fs::write(&target, b"{}").unwrap();
    let held = std::fs::File::open(&target).unwrap();
    let result = replace(dir.path(), &target);
    drop(held);
    format!("{result:?}")
}

/// Replaces `target` `ROUNDS` times while another thread reads its metadata
/// in a loop, as `layout::path_exists` does before it takes the lock.
fn with_metadata_reader(replace: fn(&Path, &Path) -> Result<(), String>) -> String {
    let dir = tempfile::tempdir().unwrap();
    let target: PathBuf = dir.path().join("marker.json");
    std::fs::write(&target, b"{}").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let reader = {
        let stop = Arc::clone(&stop);
        let target = target.clone();
        std::thread::spawn(move || {
            let mut reads = 0_u64;
            while !stop.load(Ordering::Relaxed) {
                let _ = std::fs::symlink_metadata(&target);
                reads += 1;
            }
            reads
        })
    };
    let mut failures = std::collections::BTreeMap::<String, usize>::new();
    for _ in 0..ROUNDS {
        if let Err(code) = replace(dir.path(), &target) {
            *failures.entry(code).or_default() += 1;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let reads = reader.join().unwrap();
    format!("{ROUNDS} replaces, {reads} metadata reads, failures by os error {failures:?}")
}

#[test]
fn replace_over_an_open_target() {
    let report = [
        format!("held handle, tempfile persist: {}", with_held_handle(persist)),
        format!("held handle, std rename: {}", with_held_handle(std_rename)),
        format!("metadata reader, tempfile persist: {}", with_metadata_reader(persist)),
        format!("metadata reader, std rename: {}", with_metadata_reader(std_rename)),
    ]
    .join("\n");
    println!("{report}");
    assert!(!cfg!(windows), "PROBE REPORT\n{report}");
}
