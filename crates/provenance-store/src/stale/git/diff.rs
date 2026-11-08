//! The files and line spans that a commit range changed.

use super::{checked, command, git, ChangeKind, ChangedFile, LineSpan};
use crate::stale::in_order;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};

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
    let (paths, malformed) = changed_paths(&fields);
    // Each file diff is its own git process; they run side by side. The
    // results stay in diff order, so the first failure in that order wins,
    // and a malformed entry fails after the diffs of the entries before it.
    let lines = in_order::map(paths, 1, |(kind, old_path, new_path)| {
        let lines = diff_lines(repo, base, head, old_path, new_path);
        (kind, old_path, new_path, lines)
    });
    let mut changes = Vec::with_capacity(lines.len());
    for (kind, old_path, new_path, lines) in lines {
        let (old_lines, new_lines) = lines?;
        changes.push(ChangedFile {
            old_path: Utf8PathBuf::from(old_path),
            new_path: Utf8PathBuf::from(new_path),
            kind,
            old_lines,
            new_lines,
        });
    }
    malformed.map_or_else(|| Ok(changes), Err)
}

/// The kind, old path and new path of each `--name-status -z` entry, up to
/// the first malformed entry, and the error for that entry.
fn changed_paths(fields: &[String]) -> (Vec<(ChangeKind, &str, &str)>, Option<anyhow::Error>) {
    let mut cursor = 0;
    let mut paths = Vec::new();
    while cursor < fields.len() {
        let status = &fields[cursor];
        cursor += 1;
        let Some(old_path) = fields.get(cursor) else {
            return (
                paths,
                Some(anyhow::anyhow!("git diff omitted a changed path")),
            );
        };
        cursor += 1;
        let (kind, new_path) = match status.as_bytes().first() {
            Some(b'A') => (ChangeKind::Added, old_path.as_str()),
            Some(b'D') => (ChangeKind::Deleted, old_path.as_str()),
            Some(b'R' | b'C') => {
                let Some(new_path) = fields.get(cursor) else {
                    return (
                        paths,
                        Some(anyhow::anyhow!("git diff omitted a rename destination")),
                    );
                };
                cursor += 1;
                (ChangeKind::Renamed, new_path.as_str())
            }
            _ => (ChangeKind::Modified, old_path.as_str()),
        };
        paths.push((kind, old_path.as_str(), new_path));
    }
    (paths, None)
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

pub(super) fn parse_hunk_span(field: &str, prefix: char) -> anyhow::Result<LineSpan> {
    let coordinates = field
        .strip_prefix(prefix)
        .context("malformed git diff hunk coordinates")?;
    let (start, count) = coordinates.split_once(',').unwrap_or((coordinates, "1"));
    Ok(LineSpan {
        start: start.parse()?,
        count: count.parse()?,
    })
}
