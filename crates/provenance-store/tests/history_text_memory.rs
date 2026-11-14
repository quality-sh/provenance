//! A record history read holds at most one committed text at a time, and
//! keeps no text after it ends.
//!
//! Implementation aid with no Rule binding: the test process counts the
//! bytes that it holds through its allocator. A history read of many large
//! committed texts adds less than two texts at its peak, and less than a
//! quarter of one text stays after it. The count does not depend on timing. This binary holds one test, so no other test adds
//! to the count.

mod discussion_support;
use camino::Utf8Path;
use discussion_support::*;
use provenance_core::review::ReviewHistoryQuery;
use provenance_core::NodeType;
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::repository::Repository;
use provenance_store::review::history::contract::{Commit, MemoryFileVersions};
use provenance_store::review::history::{FileVersions, GitFileVersions};
use provenance_store::review::read_history;
use provenance_store::shards;
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The system allocator, with a count of the bytes it holds and of the
/// largest count since the last reset.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::SeqCst) + size;
    PEAK.fetch_max(live, Ordering::SeqCst);
}

fn shrink(size: usize) {
    LIVE.fetch_sub(size, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc_zeroed(layout);
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
        shrink(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let moved = System.realloc(pointer, layout, size);
        if !moved.is_null() {
            if size > layout.size() {
                grow(size - layout.size());
            } else {
                shrink(layout.size() - size);
            }
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The number of committed versions of the record file.
const VERSIONS: usize = 16;
/// The size of each committed text.
const TEXT: usize = 4 << 20;

/// The committed texts: each holds the record line of `req_a` and a line
/// of other bytes, different in each version, that is not a JSON record.
fn texts(record: &str) -> Vec<String> {
    (0..VERSIONS)
        .map(|index| {
            let mut text = format!("{record}\n{index}");
            text.extend(std::iter::repeat_n('x', TEXT - text.len() - 1));
            text.push('\n');
            text
        })
        .collect()
}

/// A Git repository at `root` whose commits each hold one text at `path`.
fn git_history(root: &std::path::Path, path: &str, texts: &[String]) {
    let git = |args: &[&str]| {
        let output = provenance_store::git_process::git()
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed: {output:?}");
    };
    git(&["init", "-q"]);
    let mut import = provenance_store::git_process::git()
        .args(["fast-import", "--quiet"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = import.stdin.take().unwrap();
    for (index, text) in texts.iter().enumerate() {
        let time = 1_700_000_000 + index;
        write!(
            input,
            "commit refs/heads/main\ncommitter ann <ann@example.com> {time} +0000\ndata 1\nc\n"
        )
        .unwrap();
        write!(input, "M 100644 inline {path}\ndata {}\n", text.len()).unwrap();
        input.write_all(text.as_bytes()).unwrap();
        input.write_all(b"\n").unwrap();
    }
    drop(input);
    assert!(import.wait().unwrap().success());
    git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
}

/// The bytes that a history read of `req_a` through `versions` adds: the
/// largest count during the read, and the count that stays after it. Also
/// the number of versions that it lists.
async fn peak_of_read(
    repository: &Repository,
    versions: Arc<dyn FileVersions>,
) -> (usize, usize, usize) {
    let repository = repository.with_file_versions(versions);
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: id(),
        limit: 50,
        cursor: None,
    };
    let start = LIVE.load(Ordering::SeqCst);
    PEAK.store(start, Ordering::SeqCst);
    let entries = read_history(&repository, ReadPolicy::default(), query)
        .await
        .unwrap()
        .result
        .entries;
    let listed = entries.len();
    drop(entries);
    drop(repository);
    let kept = LIVE.load(Ordering::SeqCst).saturating_sub(start);
    (PEAK.load(Ordering::SeqCst) - start, kept, listed)
}

#[tokio::test]
async fn a_history_read_holds_one_committed_text_at_a_time() {
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = repository(root);
    let shard = shards::path_for(repository.layout(), &scope(), NodeType::Requirement);
    let relative = shard
        .strip_prefix(repository.working_tree())
        .unwrap()
        .as_str()
        .replace('\\', "/");
    let working = std::fs::read_to_string(&shard).unwrap();
    let record = working
        .lines()
        .find(|line| line.contains("\"req_a\""))
        .unwrap();
    let texts = texts(record);
    let limit = 2 * TEXT;
    assert!(VERSIONS * TEXT > 4 * limit);

    let commits = texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let name = format!("c{index}");
            let parent = index.checked_sub(1).map(|parent| format!("c{parent}"));
            let parents = parent.iter().map(String::as_str).collect::<Vec<_>>();
            Commit::new(&name, &parents, "ann", 1_700_000_000).with_file(&relative, text)
        })
        .collect::<Vec<_>>();
    let memory = Arc::new(MemoryFileVersions::new(&commits));
    drop(commits);
    let (peak, kept, listed) = peak_of_read(&repository, memory).await;
    assert_eq!(listed, 1, "the memory history lists the record once");
    assert!(
        peak < limit,
        "a read over the memory adapter added {peak} bytes; one text has {TEXT}"
    );
    assert!(
        kept < TEXT / 4,
        "a read over the memory adapter kept {kept} bytes; one text has {TEXT}"
    );

    let history = tempfile::tempdir().unwrap();
    git_history(history.path(), &relative, &texts);
    drop(texts);
    let git_root = dunce::canonicalize(history.path()).unwrap();
    let git = Arc::new(GitFileVersions::new(
        camino::Utf8PathBuf::from_path_buf(git_root).unwrap(),
    ));
    let (peak, kept, listed) = peak_of_read(&repository, git).await;
    assert_eq!(listed, 1, "the Git history lists the record once");
    assert!(
        peak < limit,
        "a read over the git adapter added {peak} bytes; one text has {TEXT}"
    );
    assert!(
        kept < TEXT / 4,
        "a read over the git adapter kept {kept} bytes; one text has {TEXT}"
    );
}
