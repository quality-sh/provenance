//! No-wildcard gates for the closed vocabularies.
//!
//! A `match` over `RelationKind` or `NodeType` in a production crate may
//! not carry a `_` arm: a kind without a declared arm has to be a compile
//! error, not a runtime filter. This gate fails when a wildcard reaches a
//! production match, so the vocabulary stays closed by the compiler, not
//! by luck. It also draws the closed-parameterization line in code: the
//! vocabulary is parameters for fixed operations, never a predicate
//! language.

use std::path::{Path, PathBuf};

const PRODUCTION_CRATES: &[&str] = &[
    "provenance-core",
    "provenance-store",
    "provenance-cli",
    "provenance-scanner",
    "provenance-sdk",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives under crates/")
        .parent()
        .expect("crates/ lives under the workspace root")
        .to_path_buf()
}

/// Every production source file: test directories, `tests.rs` files, and
/// `*_tests.rs` files are left out, and inline `#[cfg(test)]` modules are
/// stripped by `production_lines`.
fn production_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for crate_name in PRODUCTION_CRATES {
        let mut stack = vec![root.join("crates").join(crate_name).join("src")];
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
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                let is_rust = std::path::Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"));
                if is_rust && name != "tests.rs" && !name.ends_with("_tests.rs") {
                    sources.push(path);
                }
            }
        }
    }
    sources
}

/// Drops inline `#[cfg(test)] mod` blocks by brace counting, so the gate
/// reads only production lines.
fn production_lines(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut lines = source.lines().peekable();
    let mut skipping_depth: Option<i64> = None;
    while let Some(line) = lines.next() {
        if let Some(depth) = skipping_depth {
            let opens = i64::try_from(line.matches('{').count()).unwrap_or(i64::MAX);
            let closes = i64::try_from(line.matches('}').count()).unwrap_or(0);
            let remaining = depth + opens - closes;
            skipping_depth = (remaining > 0).then_some(remaining);
            continue;
        }
        if line.trim_start().starts_with("#[cfg(test)]") {
            if let Some(next) = lines.peek() {
                let next = next.trim_start();
                if next.starts_with("mod ") {
                    if next.contains('{') {
                        let opens = i64::try_from(next.matches('{').count()).unwrap_or(i64::MAX);
                        let closes = i64::try_from(next.matches('}').count()).unwrap_or(0);
                        if opens > closes {
                            skipping_depth = Some(opens - closes);
                        }
                    }
                    lines.next();
                    continue;
                }
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

/// Wildcard arms inside match bodies that name the closed vocabularies.
/// Answers the offending line numbers within `source`.
fn wildcard_arms(source: &str) -> Vec<usize> {
    let mut offenders = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while let Some(position) = source[index..].find("match ") {
        let start = index + position;
        let Some(brace_offset) = source[start..].find('{') else {
            break;
        };
        let body_start = start + brace_offset + 1;
        let mut depth = 1i64;
        let mut cursor = body_start;
        while cursor < bytes.len() && depth > 0 {
            match bytes[cursor] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            cursor += 1;
        }
        let body = &source[body_start..cursor.saturating_sub(1)];
        if body.contains("NodeType::") || body.contains("RelationKind::") {
            let body_line = source[..body_start].lines().count();
            for (offset, line) in body.lines().enumerate() {
                let arm = line.trim_start();
                if arm.starts_with("_ =>") || arm.starts_with("_ |") || arm == "_" {
                    offenders.push(body_line + offset);
                }
            }
        }
        index = body_start;
    }
    offenders
}

#[test]
fn the_scanner_sees_a_wildcard_arm_over_a_closed_vocabulary() {
    let bad = "fn f(k: NodeType) -> u8 {\n    match k {\n        NodeType::Source => 0,\n        _ => 9,\n    }\n}\n";
    assert_eq!(wildcard_arms(bad), [4]);
    let good = "fn f(k: NodeType) -> u8 {\n    match k {\n        NodeType::Source => 0,\n        NodeType::Requirement => 1,\n    }\n}\n";
    assert!(wildcard_arms(good).is_empty());
    let unrelated = "fn f(v: Option<u8>) -> u8 {\n    match v {\n        Some(v) => v,\n        _ => 0,\n    }\n}\n";
    assert!(wildcard_arms(unrelated).is_empty());
}

#[test]
fn the_scanner_ignores_matches_inside_test_modules() {
    let source = "#[cfg(test)]\nmod tests {\n    fn f(k: NodeType) -> u8 {\n        match k {\n            NodeType::Source => 0,\n            _ => 9,\n        }\n    }\n}\nfn production() {}\n";
    assert!(wildcard_arms(&production_lines(source)).is_empty());
}

#[test]
fn no_production_match_over_the_closed_vocabularies_carries_a_wildcard() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    for file in production_sources(&root) {
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in wildcard_arms(&production_lines(&source)) {
            offenders.push(format!("{}:{line}", file.display()));
        }
    }
    assert!(
        offenders.is_empty(),
        "wildcard match arms erode the closed vocabularies:\n{}",
        offenders.join("\n")
    );
}
