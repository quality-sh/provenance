//! No-wildcard gates for the closed vocabularies.
//!
//! A `match` over `RelationKind` or `NodeType` in a production crate may
//! not carry a wildcard or a named catch-all arm. A kind without a declared
//! arm has to be a compile error, so the vocabulary stays closed under
//! review. The scanner's known limits are declared in `scanner.rs`.

#[path = "no_wildcard_gates/scanner.rs"]
mod scanner;

use scanner::{production_lines, wildcard_arms};
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
/// `*_tests.rs` files are left out; inline test modules are stripped by
/// `production_lines`.
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
                let is_rust = Path::new(name)
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

// --- Scanner self-tests: each planted fixture is a defeat vector a review
// --- found, and the scanner must catch every one.

fn offenses(source: &str) -> Vec<String> {
    wildcard_arms(source)
        .into_iter()
        .map(|offender| offender.pattern)
        .collect()
}

#[test]
fn the_scanner_sees_a_plain_wildcard_arm() {
    let planted = "fn f(k: NodeType) -> u8 {\n    match k {\n        NodeType::Source => 0,\n        _ => 9,\n    }\n}\n";
    assert_eq!(offenses(planted), ["_"]);
}

#[test]
fn the_scanner_sees_a_guarded_wildcard_arm() {
    let planted = "fn f(k: NodeType, x: u8) -> u8 {\n    match k {\n        NodeType::Source => 0,\n        _ if x > 1 => 9,\n    }\n}\n";
    assert_eq!(offenses(planted), ["_ if x > 1"]);
}

#[test]
fn the_scanner_sees_a_named_catch_all_arm() {
    let planted = "fn f(k: NodeType) -> u8 {\n    match k {\n        NodeType::Source => 0,\n        other => 9,\n    }\n}\n";
    assert_eq!(offenses(planted), ["other"]);
    let guarded = "fn f(k: NodeType, x: u8) -> u8 {\n    match k {\n        NodeType::Source => 0,\n        other if x > 1 => 9,\n    }\n}\n";
    assert_eq!(offenses(guarded), ["other if x > 1"]);
}

#[test]
fn the_scanner_sees_a_wildcard_alternative_and_a_same_line_arm() {
    let alternative =
        "fn f(k: NodeType) -> u8 {\n    match k {\n        NodeType::Source | _ => 0,\n    }\n}\n";
    assert_eq!(offenses(alternative).len(), 1);
    let same_line = "fn f(k: NodeType) -> u8 {\n    match k { NodeType::Source => 0, _ => 9 }\n}\n";
    assert_eq!(offenses(same_line), ["_"]);
}

#[test]
fn the_scanner_sees_self_matches_inside_a_vocabulary_impl() {
    let planted = "impl NodeType {\n    fn f(self) -> u8 {\n        match self {\n            Self::Source => 0,\n            _ => 9,\n        }\n    }\n}\n";
    assert_eq!(offenses(planted), ["_"]);
}

#[test]
fn the_scanner_leaves_unrelated_matches_alone() {
    let unrelated = "fn f(v: Option<u8>) -> u8 {\n    match v {\n        Some(v) => v,\n        _ => 0,\n    }\n}\n";
    assert!(offenses(unrelated).is_empty());
    // An inner match over an unrelated enum, inside a vocabulary match's
    // arm expression, is judged on its own scrutinee and patterns.
    let nested = "fn f(k: NodeType, v: Option<u8>) -> u8 {\n    match k {\n        NodeType::Source => match v {\n            Some(v) => v,\n            _ => 0,\n        },\n        NodeType::Requirement => 1,\n    }\n}\n";
    assert!(
        offenses(nested).is_empty(),
        "an inner match over another enum is not this vocabulary's business"
    );
}

#[test]
fn the_scanner_ignores_test_modules_with_either_brace_style() {
    let same_line = "#[cfg(test)]\nmod tests {\n    fn f(k: NodeType) -> u8 {\n        match k {\n            NodeType::Source => 0,\n            _ => 9,\n        }\n    }\n}\nfn production() {}\n";
    assert!(offenses(&production_lines(same_line)).is_empty());
    let next_line = "#[cfg(test)]\nmod tests\n{\n    fn f(k: NodeType) -> u8 {\n        match k {\n            NodeType::Source => 0,\n            _ => 9,\n        }\n    }\n}\nfn production() {}\n";
    assert!(offenses(&production_lines(next_line)).is_empty());
    let kept = production_lines(same_line);
    assert!(kept.contains("fn production()"), "production lines stay");
}

// --- The repository gate.

#[test]
fn the_repository_scan_visits_the_production_tree() {
    let sources = production_sources(&workspace_root());
    assert!(
        sources.len() > 40,
        "the scan must visit the production tree; saw {} files",
        sources.len()
    );
    for expected in [
        "provenance-core/src/model/graph.rs",
        "provenance-core/src/model/relations.rs",
        "provenance-store/src/operations/queries/records.rs",
    ] {
        assert!(
            sources.iter().any(|path| path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with(expected)),
            "the scan must include {expected}"
        );
    }
}

#[test]
fn no_production_match_over_the_closed_vocabularies_carries_a_wildcard() {
    let root = workspace_root();
    let sources = production_sources(&root);
    assert!(!sources.is_empty(), "the scan found no sources");
    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    for file in &sources {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        scanned += 1;
        for offender in wildcard_arms(&production_lines(&source)) {
            offenders.push(format!(
                "{}:{} ({})",
                file.display(),
                offender.line,
                offender.pattern
            ));
        }
    }
    assert!(scanned > 40, "the gate must read the files it found");
    assert!(
        offenders.is_empty(),
        "wildcard or catch-all arms erode the closed vocabularies:\n{}",
        offenders.join("\n")
    );
}
