//! The git adapter of the file versions port: it reads the committed
//! versions of one file from the Git history of the working tree.

mod batch;

use super::versions::{BoxFuture, FileVersion, FileVersions};
use crate::stale::git::command;
pub(super) use batch::Failed;
use batch::{refuse_line_break, Batch};
use camino::{Utf8Path, Utf8PathBuf};
use std::cell::Cell;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::time::Duration;

/// The file versions of the Git history of one working tree.
///
/// Each call runs Git before it returns its future, so a caller runs it
/// where a blocking call is permitted. A directory outside Git, Git that
/// cannot run, or a repository with no commit gives no versions. A shallow
/// clone gives the commits it holds.
///
/// The texts come from one `git cat-file --batch` process that the adapter
/// starts at its first text. The process gives one text per call, so the
/// adapter holds at most one text. The process stops when the adapter is
/// dropped, at an error, or when the adapter reads no text for one to two
/// seconds, so an adapter that an embedder keeps does not keep the pack files
/// of the repository open. The next text starts a new process.
///
/// After a text error, a history read through this adapter asks for the
/// remaining texts of the file, and a Git failure among them wins over that
/// error, as when one Git process read every text of the file.
pub struct GitFileVersions {
    root: Utf8PathBuf,
    texts: OnceLock<Arc<Mutex<Texts>>>,
}

/// The batch process of an adapter, and what its idle stop compares.
#[derive(Debug, Default)]
struct Texts {
    batch: Option<Batch>,
    /// The number of texts read.
    reads: u64,
    /// A thread waits to stop the process when it is idle.
    watched: bool,
}

/// How long the idle stop waits between two looks at the process.
const IDLE: Duration = Duration::from_secs(1);

impl GitFileVersions {
    /// The adapter over the Git history of the working tree at `root`.
    pub const fn new(root: Utf8PathBuf) -> Self {
        Self {
            root,
            texts: OnceLock::new(),
        }
    }

    fn texts(&self) -> &Arc<Mutex<Texts>> {
        self.texts.get_or_init(Arc::default)
    }

    /// Reads one text through the batch process, which it starts when there
    /// is none. The process is out of its lock while it reads, so two reads
    /// at the same time use two processes, and the adapter keeps one of them.
    /// An error of the process stops it, so the next text starts a new one.
    /// A name that the adapter refuses does not use the process.
    fn read_text(&self, relative: &str, commit: &str) -> anyhow::Result<Option<String>> {
        refuse_line_break(relative)?;
        if !is_commit_id(commit, commit.len()) {
            return Ok(None);
        }
        let taken = lock(self.texts()).batch.take();
        let mut batch = taken.map_or_else(|| self.start(), Ok)?;
        let bytes = if is_commit_id(commit, batch.id_length()) {
            batch.bytes(commit, relative)?
        } else {
            None
        };
        self.keep(batch);
        Ok(bytes.map(String::from_utf8).transpose()?)
    }

    /// Starts the batch process. The prefix of a root and the length of its
    /// commit ids do not change, so the last version read of the root gives
    /// them when there is one.
    fn start(&self) -> anyhow::Result<Batch> {
        let known = lock(&LAST_READ)
            .as_ref()
            .filter(|read| read.root == self.root)
            .map(|read| (read.prefix.clone(), read.head.len()));
        let (prefix, id_length) = if let Some(known) = known {
            known
        } else {
            let head = repository_head(&self.root).ok_or_else(|| {
                crate::refusal::fault!("record history repository is unavailable")
            })?;
            (head.prefix, head.commit.len())
        };
        Batch::start(&self.root, prefix, id_length)
    }

    /// Puts the process back after a read, and starts the idle stop when no
    /// thread waits to stop it.
    fn keep(&self, batch: Batch) {
        let mut texts = lock(self.texts());
        texts.reads += 1;
        let other = texts.batch.replace(batch);
        if !texts.watched {
            let watched = Arc::downgrade(self.texts());
            texts.watched = std::thread::Builder::new()
                .name("git cat-file idle stop".to_owned())
                .spawn(move || stop_when_idle(&watched))
                .is_ok();
        }
        drop(texts);
        drop(other);
    }
}

/// Stops the process when no text was read between two looks, `IDLE` apart.
/// Ends when the adapter is dropped.
fn stop_when_idle(texts: &Weak<Mutex<Texts>>) {
    let mut seen = None;
    loop {
        std::thread::sleep(IDLE);
        let Some(texts) = texts.upgrade() else {
            return;
        };
        let mut held = lock(&texts);
        if seen != Some(held.reads) {
            seen = Some(held.reads);
            continue;
        }
        held.watched = false;
        let batch = held.batch.take();
        drop(held);
        drop(batch);
        return;
    }
}

/// A full commit id of a repository whose commit ids have `length` digits:
/// 40 hexadecimal digits, or 64 in a SHA-256 repository. Git reads upper-case
/// digits as the same id. Another name, such as an empty name, `HEAD` or the
/// first 40 digits of a SHA-256 id, is not a commit that the adapter holds.
fn is_commit_id(commit: &str, length: usize) -> bool {
    matches!(length, 40 | 64)
        && commit.len() == length
        && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
}

thread_local! {
    /// The last text call on this thread went to a git adapter.
    static GIT_TEXT: Cell<bool> = const { Cell::new(false) };
}

/// Calls `text`, and tells whether a git adapter gave the text on this
/// thread. The git adapter reads a text before it returns its future, so
/// the answer is known when `text` returns.
pub(super) fn answered_by_git<T>(text: impl FnOnce() -> T) -> (T, bool) {
    GIT_TEXT.set(false);
    let answer = text();
    (answer, GIT_TEXT.take())
}

/// Shows the root; the batch process is not a value of the adapter.
impl std::fmt::Debug for GitFileVersions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GitFileVersions")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// A clone starts its own batch process at its first text.
impl Clone for GitFileVersions {
    fn clone(&self) -> Self {
        Self::new(self.root.clone())
    }
}

impl FileVersions for GitFileVersions {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        let versions = file_versions(&self.root, path);
        Box::pin(std::future::ready(versions))
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        let text = self.read_text(path, commit);
        GIT_TEXT.set(true);
        Box::pin(std::future::ready(text))
    }
}

/// The versions of one record file, read for one `HEAD` and one shallow boundary.
struct ReadVersions {
    root: Utf8PathBuf,
    relative: String,
    prefix: String,
    head: String,
    shallow: Option<Vec<u8>>,
    versions: Arc<Vec<FileVersion>>,
}

// The versions of the file of the last history read. The committed versions
// change only when `HEAD` or the shallow boundary changes, so a new read of the
// same file at the same `HEAD` and boundary starts one Git process. One file is
// kept, with no text.
static LAST_READ: Mutex<Option<ReadVersions>> = Mutex::new(None);

/// Lists the commits reachable from `HEAD`, as one read resolves it, that
/// changed `relative`, parents before children.
fn file_versions(root: &Utf8Path, relative: &str) -> anyhow::Result<Arc<Vec<FileVersion>>> {
    let Some(Head {
        prefix,
        commit,
        shallow,
    }) = repository_head(root)
    else {
        return Ok(Arc::default());
    };
    let last = lock(&LAST_READ);
    if let Some(read) = last.as_ref().filter(|read| {
        read.root == root
            && read.relative == relative
            && read.head == commit
            && read.shallow == shallow
    }) {
        return Ok(read.versions.clone());
    }
    drop(last);
    // `HEAD` can move after `repository_head`, so the read names the commit.
    let versions = Arc::new(read_file_versions(root, &commit, relative)?);
    *lock(&LAST_READ) = Some(ReadVersions {
        root: root.to_owned(),
        relative: relative.to_owned(),
        prefix,
        head: commit,
        shallow,
        versions: versions.clone(),
    });
    Ok(versions)
}

fn read_file_versions(
    root: &Utf8Path,
    head: &str,
    relative: &str,
) -> anyhow::Result<Vec<FileVersion>> {
    // `--parents` names each parent as the nearest commit that changed the file.
    let output = command(root)
        .args([
            "log",
            "--reverse",
            "--topo-order",
            "--parents",
            "--format=%H%x1f%P%x1f%an%x1f%ct",
            head,
            "--",
        ])
        .arg(relative)
        .output()?;
    crate::write_error::ensure!(
        Fault,
        output.status.success(),
        "git log failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let mut versions = Vec::new();
    for line in String::from_utf8(output.stdout)?.lines() {
        let mut parts = line.split('\u{1f}');
        let (Some(commit), Some(parents), Some(author), Some(time)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(crate::refusal::fault!(
                "git log returned an unreadable commit line"
            ));
        };
        versions.push(FileVersion {
            commit: commit.to_owned(),
            parents: parents.split_whitespace().map(str::to_owned).collect(),
            author: author.to_owned(),
            committed_at: time.parse()?,
        });
    }
    Ok(versions)
}

/// The path of `root` inside its work tree, the `HEAD` commit, and the shallow
/// boundary.
struct Head {
    prefix: String,
    commit: String,
    /// The text of the `shallow` file of the repository, or `None` when the
    /// repository is complete. A fetch that makes a shallow clone deeper changes it.
    shallow: Option<Vec<u8>>,
}

/// Reads the prefix, `HEAD` and the Git directory with one Git process, then the
/// shallow boundary from the Git directory. `None` when `root` is not in a Git work
/// tree with a commit.
fn repository_head(root: &Utf8Path) -> Option<Head> {
    let output = command(root)
        .args([
            "rev-parse",
            "--git-common-dir",
            "--show-prefix",
            "--verify",
            "--quiet",
            "HEAD",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let mut lines = text.lines();
    // A relative Git directory is relative to `root`; `join` keeps an absolute one.
    let shallow = std::fs::read(root.join(lines.next()?).join("shallow")).ok();
    let prefix = lines.next()?.to_owned();
    let commit = lines.next()?.to_owned();
    Some(Head {
        prefix,
        commit,
        shallow,
    })
}

/// A cache entry holds only read results, so a poisoned lock still holds a
/// whole entry. A text read takes the batch process out of its lock while it
/// reads, so a poisoned batch lock holds a whole process or none.
fn lock<T>(cache: &Mutex<T>) -> MutexGuard<'_, T> {
    cache.lock().unwrap_or_else(PoisonError::into_inner)
}
