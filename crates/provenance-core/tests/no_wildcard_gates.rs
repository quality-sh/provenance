//! No-wildcard gates for the closed vocabularies.
//!
//! A `match` over `RelationKind` or `NodeType` in a production crate may
//! not carry a `_` arm: a family without a declared arm cannot traverse at
//! all, and that has to be a compile error, not a runtime filter. This
//! gate fails when a wildcard sneaks into a production match, so the
//! vocabulary stays closed by review, not by luck.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives in the workspace crates/ directory")
        .parent()
        .expect("crates/ lives in the workspace root")
        .to_path_buf()
}

const PRODUCTION_CRATES: &[&str] = &[
    "provenance-core",
    "provenance-store",
    "provenance-cli",
    "provenance-scanner",
    "provenance-sdk",
];

/// Strips `#[cfg(test)]`-gated inline `mod` blocks so the gate only reads
/// production code.
fn strip_test_modules(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut lines = source.lines().peekable();
    let mut in_test_block: Option<i64> = None;
    while let Some(line) = lines.next() {
        if let Some(depth) = in_test_block {
            let opens = i64::try_from(line.chars().filter(|c| *c == '{').count()).unwrap_or(0);
            let closes = i64::try_from(line.chars().filter(|c| *c == '}').count()).unwrap_or(0);
            let remaining = depth + opens - closes;
            if remaining <= 0 {
                in_test_block = None;
            }
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test") {
            if let Some(next) = lines.peek() {
                if next.trim_start().starts_with("mod ") {
                    let opens =
                        i64::try_from(next.chars().filter(|c| *c == '{').count()).unwrap_or(0);
                    let closes =
                        i64::try_from(next.chars().filter(|c| *c == '}').count()).unwrap_or(0);
                    let _ = next;
                    match (opens - closes, next.contains('{')) {
                        (0, false) => {
                            // File-referenced test module: skip its
                            // declaration only; the file itself is skipped
                            // by the path rules.
                            lines.next();
                            continue;
                        }
                        (delta, true) => {
                            in_test_block = Some(delta);
                            lines.next();
                            continue;
                        }
                        _ => {
                            lines.next();
                            continue;
                        }
                    }
                }
            }
            continue;
        }
        let _ = writeln!(output, "{line}");
    }
    output
}

/// Finds wildcard arms inside match blocks whose arms reference the
/// closed vocabularies. Returns the offending (file, line) pairs.
fn wildcard_arms(file: &Path, source: &str) -> Vec<(String, usize)> {
    let mut offenders = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while let Some(position) = source[index..].find("match ") {
        let start = index + position;
        // Scrutinee ends at the opening brace of the match body.
        let Some(brace_offset) = source[start..].find('{') else {
            break;
        };
        let body_start = start + brace_offset + 1;
        let mut depth = 1i64;
        let mut cursor = body_start;
        while cursor < bytes.len() && depth > 0 {
            if bytes[cursor] == b'{' {
                depth += 1;
            } else if bytes[cursor] == b'}' {
                depth -= 1;
            }
            cursor += 1;
        }
        let body = &source[body_start..cursor.saturating_sub(1)];
        let references_vocabulary = body.contains("NodeType::") || body.contains("RelationKind::");
        if references_vocabulary {
            let line_number = source[..body_start].lines().count();
            for (offset, line) in body.lines().enumerate() {
                let arm = line.trim_start();
                if arm.starts_with("_ =>") || arm.starts_with("_|") || arm == "_" {
                    offenders.push((file.display().to_string(), line_number + offset));
                }
            }
        }
        index = body_start.max(index + 1);
    }
    offenders
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for crate_name in PRODUCTION_CRATES {
        let src = root.join("crates").join(crate_name).join("src");
        let mut stack = vec![src];
        while let Some(directory) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|name| name == "tests") {
                        continue;
                    }
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    sources.push(path);
                }
            }
        }
    }
    sources
}

#[test]
fn no_production_match_over_the_closed_vocabularies_carries_a_wildcard() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    for file in rust_sources(&root) {
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        let production = strip_test_modules(&source);
        offenders.extend(wildcard_arms(&file, &production));
    }
    assert!(
        offenders.is_empty(),
        "wildcard match arms erode the closed vocabularies: {offenders:?}"
    );
}
