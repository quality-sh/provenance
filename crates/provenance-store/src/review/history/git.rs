//! Reads the committed versions of one graph record file from Git.

use crate::stale::git::command;
use camino::{Utf8Path, Utf8PathBuf};
use std::io::Write;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

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
// kept, so the memory stays at the size of one history read.
static LAST_READ: Mutex<Option<ReadVersions>> = Mutex::new(None);

/// One commit that changed a record file, with the file text at that commit.
pub(super) struct FileVersion {
    pub(super) commit: String,
    /// The nearest listed commits on each parent line, in parent order. The
    /// file text at each is the file text at the matching parent. A root
    /// commit, or a commit at the edge of a shallow clone, has none.
    pub(super) parents: Vec<String>,
    pub(super) author: String,
    pub(super) committed_at: i64,
    /// `None` when the commit removed the file.
    pub(super) text: Option<String>,
}

/// Lists the commits reachable from `HEAD`, as one read resolves it, that changed `path`, parents before
/// children.
///
/// A directory outside Git, Git that cannot run, or a repository with no
/// commit gives no versions. A shallow clone gives the commits it holds.
pub(super) fn file_versions(
    root: &Utf8Path,
    path: &Utf8Path,
) -> anyhow::Result<Arc<Vec<FileVersion>>> {
    let Some(Head {
        prefix,
        commit,
        shallow,
    }) = repository_head(root)
    else {
        return Ok(Arc::default());
    };
    let relative = path.strip_prefix(root)?.as_str().replace('\\', "/");
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
    let versions = Arc::new(read_file_versions(root, &commit, &prefix, &relative)?);
    *lock(&LAST_READ) = Some(ReadVersions {
        root: root.to_owned(),
        relative,
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
    prefix: &str,
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
    anyhow::ensure!(
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
            anyhow::bail!("git log returned an unreadable commit line");
        };
        versions.push(FileVersion {
            commit: commit.to_owned(),
            parents: parents.split_whitespace().map(str::to_owned).collect(),
            author: author.to_owned(),
            committed_at: time.parse()?,
            text: None,
        });
    }
    if versions.is_empty() {
        return Ok(versions);
    }
    let commits = versions
        .iter()
        .map(|version| version.commit.as_str())
        .collect::<Vec<_>>();
    let texts = read_blobs(root, &blob_names(&commits, prefix, relative), commits.len())?;
    for (version, text) in versions.iter_mut().zip(texts) {
        version.text = text;
    }
    Ok(versions)
}

/// The `git cat-file --batch` input that names the file at each revision.
fn blob_names(revisions: &[&str], prefix: &str, relative: &str) -> String {
    let mut names = String::new();
    for revision in revisions {
        names.push_str(revision);
        names.push(':');
        names.push_str(prefix);
        names.push_str(relative);
        names.push('\n');
    }
    names
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

/// Reads each named blob through one `git cat-file --batch` process.
fn read_blobs(root: &Utf8Path, names: &str, count: usize) -> anyhow::Result<Vec<Option<String>>> {
    let mut child = command(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let input = names.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output()?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("git cat-file input thread failed"))??;
    anyhow::ensure!(
        output.status.success(),
        "git cat-file failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let mut rest = output.stdout.as_slice();
    let mut texts = Vec::with_capacity(count);
    for _ in 0..count {
        let end = rest
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| anyhow::anyhow!("git cat-file output ended early"))?;
        let header = std::str::from_utf8(&rest[..end])?;
        rest = &rest[end + 1..];
        if header.ends_with(" missing") {
            texts.push(None);
            continue;
        }
        let size = header
            .rsplit(' ')
            .next()
            .and_then(|size| size.parse::<usize>().ok())
            .ok_or_else(|| anyhow::anyhow!("git cat-file returned an unreadable header"))?;
        anyhow::ensure!(rest.len() > size, "git cat-file output ended early");
        texts.push(Some(String::from_utf8(rest[..size].to_vec())?));
        rest = &rest[size + 1..];
    }
    Ok(texts)
}

/// A cache entry holds only read results, so a poisoned lock still holds a
/// whole entry.
fn lock<T>(cache: &Mutex<T>) -> MutexGuard<'_, T> {
    cache.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The texts of one file at named commits, from the last read.
struct ReadTexts {
    root: Utf8PathBuf,
    relative: String,
    commits: Vec<String>,
    texts: Vec<Option<String>>,
}

// The file at a named commit does not change, so a new read of the same file
// at the same commits starts no Git process.
static LAST_TEXTS: Mutex<Option<ReadTexts>> = Mutex::new(None);

/// Reads one record file at each specified commit through one Git process.
pub(super) fn file_at_commits(
    root: &Utf8Path,
    path: &Utf8Path,
    commits: &[&str],
) -> anyhow::Result<Vec<Option<String>>> {
    if commits.is_empty() {
        return Ok(Vec::new());
    }
    let relative = path.strip_prefix(root)?.as_str().replace('\\', "/");
    if let Some(read) = lock(&LAST_TEXTS)
        .as_ref()
        .filter(|read| read.root == root && read.relative == relative && read.commits == commits)
    {
        return Ok(read.texts.clone());
    }
    // The prefix of a root does not change, so the last history read gives it.
    let known = lock(&LAST_READ)
        .as_ref()
        .filter(|read| read.root == root)
        .map(|read| read.prefix.clone());
    let prefix = match known {
        Some(prefix) => prefix,
        None => {
            repository_head(root)
                .ok_or_else(|| anyhow::anyhow!("record history repository is unavailable"))?
                .prefix
        }
    };
    let texts = read_blobs(
        root,
        &blob_names(commits, &prefix, &relative),
        commits.len(),
    )?;
    *lock(&LAST_TEXTS) = Some(ReadTexts {
        root: root.to_owned(),
        relative,
        commits: commits.iter().map(|commit| (*commit).to_owned()).collect(),
        texts: texts.clone(),
    });
    Ok(texts)
}
