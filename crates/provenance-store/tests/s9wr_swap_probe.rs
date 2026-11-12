#![allow(dead_code, unused_imports, clippy::all, clippy::pedantic)]
//! Throwaway probe for provenance-s9wr. Not for merge. It counts how often a
//! staged write (one whole set, a swap of the state directory) fails while a
//! reader in a second thread has files or directories of the state open. The
//! test always passes; it prints the counts. `.config/nextest.toml` on the
//! probe branch shows the output of this test when it passes.

#[path = "review_support/mod.rs"]
mod base;

use base::*;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

const TRIES: usize = 400;

#[derive(Clone, Copy, Debug)]
enum Reader {
    /// No second thread.
    None,
    /// Opens, reads and closes each file below the state directory.
    ReadFiles,
    /// Lists each directory below the state directory.
    ListDirectories,
    /// Opens the repository through the public call, as a second command does.
    OpenRepository,
    /// Opens a full view through the public call (reads under the lock).
    ReadView,
    /// The writer keeps the thread and message shards open during each write.
    HeldShards,
    /// A thread keeps every state file open for 5 ms, closes, and opens again.
    HeldLoop,
    /// The writer keeps a handle on the state directory itself, as the
    /// recursive watch of the review host does.
    HeldStateDir,
    /// The writer keeps a handle on `state/scopes`, a directory below.
    HeldSubdir,
}

/// Opens a directory handle. Windows needs backup semantics for a
/// directory; the share mode of `std` lets other handles read, write and
/// delete.
fn open_directory(path: &Utf8Path) -> std::fs::File {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0200_0000);
    }
    options.open(path).unwrap()
}

fn message(body: &str) -> provenance_store::state_store::PostMessageInput {
    serde_json::from_value(serde_json::json!({
        "scope_id": "default",
        "parent": {"node_type": "requirement", "node_id": "req_a"},
        "role": "user", "body": body
    }))
    .unwrap()
}

fn files_below(directory: &Utf8Path, files: &mut Vec<Utf8PathBuf>, dirs: &mut Vec<Utf8PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    dirs.push(directory.to_owned());
    for entry in entries.flatten() {
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => files_below(&path, files, dirs),
            Ok(kind) if kind.is_file() => files.push(path),
            _ => {}
        }
    }
}

fn message_shards(state: &Utf8Path) -> Vec<Utf8PathBuf> {
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    files_below(state, &mut files, &mut dirs);
    files
        .into_iter()
        .filter(|path| {
            let text = path.as_str();
            text.contains("thread") || text.contains("message")
        })
        .collect()
}

/// A short class of a failure: the step and the OS error code.
fn class(text: &str) -> String {
    let step = [
        "move live state",
        "install staged state",
        "return live state to stage during rollback",
        "restore backup state during rollback",
        "failed to persist temporary file",
        "sync publication file",
        "list publication tree",
        "publication recovery",
    ]
    .into_iter()
    .find(|step| text.contains(step))
    .unwrap_or("other");
    let code = text
        .rfind("os error ")
        .map(|at| {
            text[at + 9..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .unwrap_or_default();
    format!("{step} (os error {code})")
}

fn spawn_reader(
    reader: Reader,
    root: Utf8PathBuf,
    stop: Arc<AtomicBool>,
    errors: Arc<AtomicU64>,
) -> Option<std::thread::JoinHandle<u64>> {
    let layout = ProvenanceLayout::new(root.clone());
    let state = layout.state_dir();
    let body: Box<dyn Fn() + Send> = match reader {
        Reader::None | Reader::HeldShards | Reader::HeldStateDir | Reader::HeldSubdir => return None,
        Reader::ReadFiles => Box::new(move || {
            let (mut files, mut dirs) = (Vec::new(), Vec::new());
            files_below(&state, &mut files, &mut dirs);
            for file in files {
                let _ = std::fs::read(file);
            }
        }),
        Reader::ListDirectories => Box::new(move || {
            let (mut files, mut dirs) = (Vec::new(), Vec::new());
            files_below(&state, &mut files, &mut dirs);
        }),
        Reader::OpenRepository => {
            let errors = Arc::clone(&errors);
            Box::new(move || {
                if Repository::open(root.as_std_path(), "default").is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                }
            })
        }
        Reader::ReadView => {
            let errors = Arc::clone(&errors);
            Box::new(move || {
                let read = Repository::open(root.as_std_path(), "default")
                    .map_err(|error| anyhow::anyhow!("{error}"))
                    .and_then(|repository| repository.view().map(|_| ()));
                if read.is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                }
            })
        }
        Reader::HeldLoop => Box::new(move || {
            let (mut files, mut dirs) = (Vec::new(), Vec::new());
            files_below(&state, &mut files, &mut dirs);
            let held: Vec<_> = files
                .iter()
                .filter_map(|file| std::fs::File::open(file).ok())
                .collect();
            std::thread::sleep(std::time::Duration::from_millis(5));
            drop(held);
        }),
    };
    Some(std::thread::spawn(move || {
        let mut rounds = 0_u64;
        while !stop.load(Ordering::Relaxed) {
            body();
            rounds += 1;
        }
        rounds
    }))
}

fn scenario(reader: Reader) -> String {
    let (temp, store) = fixture();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    store.post_thread_message(message("first")).unwrap();
    let repository = Repository::open(root.as_std_path(), "default").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let reader_errors = Arc::new(AtomicU64::new(0));
    let thread = spawn_reader(
        reader,
        root.clone(),
        Arc::clone(&stop),
        Arc::clone(&reader_errors),
    );
    let mut failures = BTreeMap::<String, (usize, String)>::new();
    let mut after_failure = BTreeMap::<String, usize>::new();
    let mut leftover_after_success = 0_usize;
    let mut passed = 0_usize;
    let started = std::time::Instant::now();
    for index in 0..TRIES {
        let held: Vec<_> = match reader {
            Reader::HeldShards => message_shards(&layout.state_dir())
                .iter()
                .map(|path| std::fs::File::open(path).unwrap())
                .collect(),
            Reader::HeldStateDir => vec![open_directory(&layout.state_dir())],
            Reader::HeldSubdir => vec![open_directory(&layout.scopes_dir())],
            _ => Vec::new(),
        };
        let result = repository.stage(false, None, |staged| {
            staged
                .post_thread_message(message(&format!("message {index}")))
                .map(|_| ())
        });
        drop(held);
        match result {
            Ok(()) => {
                passed += 1;
                if layout.publication_marker_path().exists() {
                    leftover_after_success += 1;
                }
            }
            Err(error) => {
                let text = format!("{error:#}");
                let entry = failures.entry(class(&text)).or_insert((0, text));
                entry.0 += 1;
                let state = format!(
                    "state_dir={} marker={}",
                    layout.state_dir().exists(),
                    layout.publication_marker_path().exists()
                );
                *after_failure.entry(state).or_default() += 1;
            }
        }
    }
    let elapsed = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    let rounds = thread.map_or(0, |thread| thread.join().unwrap());
    let messages = Repository::open(root.as_std_path(), "default")
        .map_err(|error| format!("{error}"))
        .and_then(|repository| {
            repository
                .store()
                .list_messages(&scope())
                .map_err(|error| format!("{error:#}"))
        })
        .map(|messages| messages.len());
    let marker_at_end = layout.publication_marker_path().exists();
    let mut report = format!(
        "S9WR {reader:?}: {TRIES} tries, {passed} passed, {} failed, {:.1} s; reader rounds {rounds}, reader errors {}; \
         messages after = {messages:?} (expected {}); marker left after a success {leftover_after_success}; marker at end {marker_at_end}",
        TRIES - passed,
        elapsed.as_secs_f64(),
        reader_errors.load(Ordering::Relaxed),
        passed + 1,
    );
    for (class, (count, sample)) in &failures {
        report.push_str(&format!("\nS9WR {reader:?}:   {count} x {class}; sample: {sample}"));
    }
    for (state, count) in &after_failure {
        report.push_str(&format!("\nS9WR {reader:?}:   after a failure: {state} in {count}"));
    }
    report
}

#[test]
fn s9wr_swap_probe() {
    let report = [
        Reader::None,
        Reader::ReadFiles,
        Reader::ListDirectories,
        Reader::OpenRepository,
        Reader::ReadView,
        Reader::HeldLoop,
        Reader::HeldShards,
        Reader::HeldStateDir,
        Reader::HeldSubdir,
    ]
    .into_iter()
    .map(scenario)
    .collect::<Vec<_>>()
    .join("\n");
    println!("S9WR PROBE os={}\n{report}", std::env::consts::OS);
    eprintln!("S9WR PROBE os={}\n{report}", std::env::consts::OS);
}
