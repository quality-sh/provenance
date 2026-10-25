//! Reads the committed versions of one graph record file from Git.

use crate::stale::git::command;
use camino::Utf8Path;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::Stdio;

/// One commit that changed a record file, with the file text at that commit
/// and at each of its parents.
pub(super) struct FileVersion {
    pub(super) commit: String,
    pub(super) author: String,
    pub(super) committed_at: i64,
    /// `None` when the commit removed the file.
    pub(super) text: Option<String>,
    /// The file text at each parent, in parent order; `None` where the parent
    /// has no such file. A root commit has no parents.
    pub(super) parent_texts: Vec<Option<String>>,
}

/// Lists the commits reachable from `HEAD` that changed `path`, parents before
/// children.
///
/// A directory outside Git, Git that cannot run, or a repository with no
/// commit gives no versions. A shallow clone gives the commits it holds.
pub(super) fn file_versions(root: &Utf8Path, path: &Utf8Path) -> anyhow::Result<Vec<FileVersion>> {
    let Some(prefix) = repository_prefix(root) else {
        return Ok(Vec::new());
    };
    let relative = path.strip_prefix(root)?.as_str().replace('\\', "/");
    let output = command(root)
        .args([
            "log",
            "--reverse",
            "--topo-order",
            "--format=%H%x1f%P%x1f%an%x1f%ct",
            "--",
        ])
        .arg(&relative)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "git log failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let mut commits = Vec::new();
    for line in String::from_utf8(output.stdout)?.lines() {
        let mut parts = line.split('\u{1f}');
        let (Some(commit), Some(parents), Some(author), Some(time)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            anyhow::bail!("git log returned an unreadable commit line");
        };
        let parents = parents
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        commits.push((
            commit.to_owned(),
            parents,
            author.to_owned(),
            time.parse::<i64>()?,
        ));
    }
    if commits.is_empty() {
        return Ok(Vec::new());
    }
    // Each distinct blob is read once: the file at every listed commit and at
    // every parent of one.
    let mut revisions = Vec::new();
    let mut seen = BTreeSet::new();
    for (commit, parents, _, _) in &commits {
        for revision in std::iter::once(commit).chain(parents) {
            if seen.insert(revision.as_str()) {
                revisions.push(revision.as_str());
            }
        }
    }
    let texts = read_blobs(
        root,
        &blob_names(&revisions, &prefix, &relative),
        revisions.len(),
    )?;
    let texts = revisions.into_iter().zip(texts).collect::<BTreeMap<_, _>>();
    let text_at = |revision: &str| texts.get(revision).cloned().flatten();
    Ok(commits
        .iter()
        .map(|(commit, parents, author, committed_at)| FileVersion {
            text: text_at(commit),
            parent_texts: parents.iter().map(|parent| text_at(parent)).collect(),
            commit: commit.clone(),
            author: author.clone(),
            committed_at: *committed_at,
        })
        .collect())
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

/// The path of `root` inside its work tree, or `None` when `root` is not in
/// a Git work tree with a commit.
fn repository_prefix(root: &Utf8Path) -> Option<String> {
    let head = command(root)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()
        .ok()?;
    if !head.status.success() {
        return None;
    }
    let output = command(root)
        .args(["rev-parse", "--show-prefix"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
        .map(|prefix| prefix.trim_end_matches(['\n', '\r']).to_owned())
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

/// Reads one record file at each specified commit through one Git process.
pub(super) fn file_at_commits(
    root: &Utf8Path,
    path: &Utf8Path,
    commits: &[&str],
) -> anyhow::Result<Vec<Option<String>>> {
    if commits.is_empty() {
        return Ok(Vec::new());
    }
    let prefix = repository_prefix(root)
        .ok_or_else(|| anyhow::anyhow!("record history repository is unavailable"))?;
    let relative = path.strip_prefix(root)?.as_str().replace('\\', "/");
    read_blobs(
        root,
        &blob_names(commits, &prefix, &relative),
        commits.len(),
    )
}
