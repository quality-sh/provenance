use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use std::process::{Command, Output};

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
    Ok((resolve_commit(repo, &base)?, resolve_commit(repo, &head)?))
}

pub fn revision_files(repo: &Utf8Path, revision: &str) -> anyhow::Result<Vec<RevisionFile>> {
    let output = git(repo, &["ls-tree", "-rz", "--name-only", revision])?;
    let mut files = Vec::new();
    for raw_path in output.stdout.split(|byte| *byte == 0) {
        if raw_path.is_empty() {
            continue;
        }
        let path = Utf8PathBuf::from(String::from_utf8(raw_path.to_vec())?);
        if path
            .extension()
            .and_then(provenance_scanner::Language::from_extension)
            .is_none()
        {
            continue;
        }
        let object = format!("{revision}:{path}");
        let blob = git(repo, &["show", &object])?;
        files.push(RevisionFile {
            path,
            content: String::from_utf8(blob.stdout)?,
        });
    }
    Ok(files)
}

pub fn changed_files(repo: &Utf8Path, base: &str, head: &str) -> anyhow::Result<Vec<ChangedFile>> {
    let output = git(
        repo,
        &["diff", "--name-status", "-z", "--find-renames", base, head],
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
    let mut command = Command::new("git");
    command
        .args([
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
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
    let output = git(repo, &["rev-parse", "--verify", &expression])?;
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn git(repo: &Utf8Path, args: &[&str]) -> anyhow::Result<Output> {
    let output = Command::new("git")
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
}
