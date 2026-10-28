use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

#[derive(Debug)]
pub struct RevisionFile {
    pub path: Utf8PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSpan {
    pub start: usize,
    pub count: usize,
}

impl LineSpan {
    pub const fn intersects(self, start: usize, end: usize) -> bool {
        self.count > 0 && self.start <= end && start < self.start + self.count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug)]
pub struct ChangedFile {
    pub old_path: Utf8PathBuf,
    pub new_path: Utf8PathBuf,
    pub kind: ChangeKind,
    pub old_lines: Vec<LineSpan>,
    pub new_lines: Vec<LineSpan>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitRefusal {
    #[error("Git capability unavailable")]
    Unavailable {
        #[source]
        source: anyhow::Error,
    },
    #[error("git rev-parse failed: {diagnostic}")]
    RevisionNotFound { diagnostic: String },
}

/// Git control data and the executable are trusted; request environment overrides are removed.
pub(crate) fn command(repo: &Utf8Path) -> Command {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
        {
            command.env_remove(key);
        }
    }
    command.arg("--no-pager");
    if no_lazy_fetch_supported() {
        command.arg("--no-lazy-fetch");
    }
    command
        .args(["-c", "core.fsmonitor=false"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .current_dir(repo);
    command
}

/// `--no-lazy-fetch` (git 2.47+) blocks implicit promisor fetches while
/// provenance reads a partial clone. The probe runs once per process, so
/// modern git keeps that block and older git keeps working.
fn no_lazy_fetch_supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| match Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            version_supports_no_lazy_fetch(&String::from_utf8_lossy(&output.stdout))
        }
        // If the version cannot be read, keep the flag: modern git must not
        // lose it, and old git then fails loudly instead of fetching.
        _ => true,
    })
}

fn version_supports_no_lazy_fetch(version: &str) -> bool {
    let Some(rest) = version.trim().strip_prefix("git version ") else {
        return true;
    };
    let mut numbers = rest
        .split(|character: char| !character.is_ascii_digit())
        .filter(|field| !field.is_empty())
        .map(str::parse::<u32>);
    match (numbers.next(), numbers.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor) >= (2, 47),
        _ => true,
    }
}

pub fn resolve_range(
    repo: &Utf8Path,
    base: Option<String>,
    head: Option<String>,
    since: Option<String>,
) -> anyhow::Result<(String, String)> {
    let (base, head) = match (base, head, since) {
        (Some(base), Some(head), None) => (base, head),
        (None, None, Some(since)) => (since, "HEAD".to_string()),
        _ => anyhow::bail!(
            "stale requires two commits (`stale <BASE> <HEAD>`) or `--since <COMMIT>`"
        ),
    };
    let available = command(repo)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(|error| GitRefusal::Unavailable {
            source: error.into(),
        })?;
    if !available.status.success() {
        return Err(GitRefusal::Unavailable {
            source: anyhow::anyhow!(
                "git rev-parse failed: {}",
                String::from_utf8_lossy(&available.stderr).trim()
            ),
        }
        .into());
    }
    Ok((resolve_commit(repo, &base)?, resolve_commit(repo, &head)?))
}

pub fn revision_files(repo: &Utf8Path, revision: &str) -> anyhow::Result<Vec<RevisionFile>> {
    read_blobs(repo, revision_source_blobs(repo, revision)?)
}

/// Read the revision blobs for the selected repository paths only. A path
/// uses `/` separators, as Git writes it. An empty selection reads no file.
pub fn revision_files_at_paths(
    repo: &Utf8Path,
    revision: &str,
    paths: &BTreeSet<String>,
) -> anyhow::Result<Vec<RevisionFile>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let blobs = revision_source_blobs(repo, revision)?
        .into_iter()
        .filter(|(_, path)| paths.contains(path.as_str()))
        .collect();
    read_blobs(repo, blobs)
}

/// The object id and path of each source blob in the revision tree. One
/// `ls-tree` call lists the tree; it takes no path specs, so the argument
/// list stays short for any selection.
fn revision_source_blobs(
    repo: &Utf8Path,
    revision: &str,
) -> anyhow::Result<Vec<(String, Utf8PathBuf)>> {
    let output = git(repo, &["ls-tree", "-rz", revision])?;
    let mut blobs = Vec::new();
    for entry in output.stdout.split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        let entry = std::str::from_utf8(entry).context("git ls-tree wrote a non-UTF-8 entry")?;
        let (meta, path) = entry
            .split_once('\t')
            .context("git ls-tree wrote an entry without a path")?;
        let mut fields = meta.split(' ');
        let (Some(_mode), Some(kind), Some(object)) = (fields.next(), fields.next(), fields.next())
        else {
            anyhow::bail!("git ls-tree wrote a malformed entry: {entry}");
        };
        let path = Utf8PathBuf::from(path);
        if kind != "blob"
            || path
                .extension()
                .and_then(provenance_scanner::Language::from_extension)
                .is_none()
        {
            continue;
        }
        blobs.push((object.to_string(), path));
    }
    Ok(blobs)
}

/// Read the blobs through one `git cat-file --batch` process.
fn read_blobs(
    repo: &Utf8Path,
    blobs: Vec<(String, Utf8PathBuf)>,
) -> anyhow::Result<Vec<RevisionFile>> {
    if blobs.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = command(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("run git cat-file --batch")?;
    let mut stdin = child.stdin.take().context("open git cat-file input")?;
    let mut request = String::new();
    for (object, _) in &blobs {
        request.push_str(object);
        request.push('\n');
    }
    let writer = std::thread::spawn(move || stdin.write_all(request.as_bytes()));
    let output = child
        .wait_with_output()
        .context("read git cat-file --batch")?;
    let written = writer
        .join()
        .map_err(|_| anyhow::anyhow!("git cat-file input writer panicked"))?;
    let output = checked(output, "git cat-file --batch")?;
    written.context("write git cat-file input")?;
    let mut rest = output.stdout.as_slice();
    let mut files = Vec::with_capacity(blobs.len());
    for (object, path) in blobs {
        let header_end = rest
            .iter()
            .position(|byte| *byte == b'\n')
            .context("git cat-file omitted an object header")?;
        let header = std::str::from_utf8(&rest[..header_end])?;
        let size = match header.split(' ').collect::<Vec<_>>().as_slice() {
            [id, "blob", size] if *id == object => size.parse::<usize>()?,
            _ => anyhow::bail!("git cat-file could not read {path} ({header})"),
        };
        let body_start = header_end + 1;
        let body = rest
            .get(body_start..body_start + size)
            .context("git cat-file wrote a short object")?;
        files.push(RevisionFile {
            content: String::from_utf8(body.to_vec())
                .with_context(|| format!("read blob {path} as UTF-8"))?,
            path,
        });
        rest = rest
            .get(body_start + size + 1..)
            .context("git cat-file omitted an object terminator")?;
    }
    Ok(files)
}

pub fn changed_files(repo: &Utf8Path, base: &str, head: &str) -> anyhow::Result<Vec<ChangedFile>> {
    let output = git(
        repo,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--name-status",
            "-z",
            "--find-renames",
            base,
            head,
            "--",
        ],
    )?;
    let fields = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8(field.to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut cursor = 0;
    let mut changes = Vec::new();
    while cursor < fields.len() {
        let status = &fields[cursor];
        cursor += 1;
        let old_path = fields
            .get(cursor)
            .context("git diff omitted a changed path")?;
        cursor += 1;
        let (kind, new_path) = match status.as_bytes().first() {
            Some(b'A') => (ChangeKind::Added, old_path.as_str()),
            Some(b'D') => (ChangeKind::Deleted, old_path.as_str()),
            Some(b'R' | b'C') => {
                let new_path = fields
                    .get(cursor)
                    .context("git diff omitted a rename destination")?;
                cursor += 1;
                (ChangeKind::Renamed, new_path.as_str())
            }
            _ => (ChangeKind::Modified, old_path.as_str()),
        };
        let (old_lines, new_lines) = diff_lines(repo, base, head, old_path, new_path)?;
        changes.push(ChangedFile {
            old_path: Utf8PathBuf::from(old_path),
            new_path: Utf8PathBuf::from(new_path),
            kind,
            old_lines,
            new_lines,
        });
    }
    Ok(changes)
}

fn diff_lines(
    repo: &Utf8Path,
    base: &str,
    head: &str,
    old_path: &str,
    new_path: &str,
) -> anyhow::Result<(Vec<LineSpan>, Vec<LineSpan>)> {
    let mut command = command(repo);
    command
        .args([
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            base,
            head,
            "--",
            old_path,
        ])
        .current_dir(repo);
    if new_path != old_path {
        command.arg(new_path);
    }
    let output = checked(command.output().context("run git diff")?, "git diff")?;
    let text = String::from_utf8(output.stdout)?;
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    for line in text.lines().filter(|line| line.starts_with("@@ ")) {
        let mut fields = line.split_whitespace();
        let _marker = fields.next();
        let old = fields.next().context("malformed git diff hunk")?;
        let new = fields.next().context("malformed git diff hunk")?;
        old_lines.push(parse_hunk_span(old, '-')?);
        new_lines.push(parse_hunk_span(new, '+')?);
    }
    Ok((old_lines, new_lines))
}

fn parse_hunk_span(field: &str, prefix: char) -> anyhow::Result<LineSpan> {
    let coordinates = field
        .strip_prefix(prefix)
        .context("malformed git diff hunk coordinates")?;
    let (start, count) = coordinates.split_once(',').unwrap_or((coordinates, "1"));
    Ok(LineSpan {
        start: start.parse()?,
        count: count.parse()?,
    })
}

fn resolve_commit(repo: &Utf8Path, revision: &str) -> anyhow::Result<String> {
    let expression = format!("{revision}^{{commit}}");
    let output = command(repo)
        .args(["rev-parse", "--verify", "--end-of-options", &expression])
        .output()
        .map_err(|error| GitRefusal::Unavailable {
            source: error.into(),
        })?;
    if !output.status.success() {
        return Err(GitRefusal::RevisionNotFound {
            diagnostic: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn git(repo: &Utf8Path, args: &[&str]) -> anyhow::Result<Output> {
    let output = command(repo)
        .args(args)
        .current_dir(repo)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    checked(output, &format!("git {}", args.join(" ")))
}

fn checked(output: Output, operation: &str) -> anyhow::Result<Output> {
    anyhow::ensure!(
        output.status.success(),
        "{operation} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output)
}

#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod security_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zero_length_and_single_line_hunks() {
        assert_eq!(
            parse_hunk_span("-4,0", '-').unwrap(),
            LineSpan { start: 4, count: 0 }
        );
        assert_eq!(
            parse_hunk_span("+7", '+').unwrap(),
            LineSpan { start: 7, count: 1 }
        );
    }

    #[test]
    fn no_lazy_fetch_decision_tracks_the_local_git_capability() {
        // Modern git keeps the block on implicit promisor fetches.
        for version in ["2.47.0", "2.47.1", "2.48.0", "2.100.0", "3.0.0"] {
            assert!(
                version_supports_no_lazy_fetch(&format!("git version {version}")),
                "git {version} must keep the flag"
            );
        }
        assert!(version_supports_no_lazy_fetch(
            "git version 2.47.0.windows.1"
        ));
        // Older git drops the flag so its commands keep running.
        for version in ["2.9", "2.43.0", "2.46.2"] {
            assert!(
                !version_supports_no_lazy_fetch(&format!("git version {version}")),
                "git {version} must drop the flag"
            );
        }
        // Unparseable output keeps the flag rather than losing it.
        assert!(version_supports_no_lazy_fetch("git"));
        assert!(version_supports_no_lazy_fetch(""));
    }
}
