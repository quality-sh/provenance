//! The shape of the store crate: catalog operations name no storage module,
//! and the Repository handle keeps its methods in one file.

use provenance_macros::verifies;
use std::fs;
use std::path::{Path, PathBuf};

/// Modules and crates that hold storage mechanics. A catalog body reaches
/// state through the `Repository` handle and its stores, never through these.
const STORAGE_MODULES: &[&str] = &["shards", "jsonl", "layout", "publication", "storage"];
const CACHE_STORAGE_MODULES: &[&str] = &["connection", "materialize"];
const STORAGE_CRATES: &[&str] = &["fs2", "sqlx", "tempfile"];

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn is_test_file(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap();
    relative
        .components()
        .any(|part| part.as_os_str() == "tests")
        || relative
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("tests.rs"))
}

/// The source without line comments and without each `#[cfg(test)]` item.
fn production_source(source: &str) -> String {
    let uncommented: String = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut kept = String::new();
    let mut rest = uncommented.as_str();
    while let Some(start) = rest.find("#[cfg(test)]") {
        kept.push_str(&rest[..start]);
        rest = skip_item(&rest[start + "#[cfg(test)]".len()..]);
    }
    kept.push_str(rest);
    kept
}

/// The text after the next item: up to its `;`, or past its balanced braces.
fn skip_item(text: &str) -> &str {
    let mut depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            ';' if depth == 0 => return &text[index + 1..],
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[index + 1..];
                }
            }
            _ => {}
        }
    }
    ""
}

/// Each `crate::` path in the source, with its braced group, as a list of
/// path segments.
fn crate_paths(source: &str) -> Vec<Vec<String>> {
    let mut paths = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("crate::") {
        let tail = &rest[start + "crate::".len()..];
        let mut depth = 0usize;
        let end = tail
            .char_indices()
            .find(|&(_, character)| match character {
                '{' => {
                    depth += 1;
                    false
                }
                '}' if depth > 0 => {
                    depth -= 1;
                    false
                }
                ',' | ' ' | '\n' if depth > 0 => false,
                character => !(character.is_alphanumeric() || character == '_' || character == ':'),
            })
            .map_or(tail.len(), |(index, _)| index);
        paths.push(
            tail[..end]
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|segment| !segment.is_empty())
                .map(str::to_owned)
                .collect(),
        );
        rest = &tail[end..];
    }
    paths
}

fn words(source: &str) -> impl Iterator<Item = &str> {
    source
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
}

#[test]
#[verifies("rule_catalog_operations_name_no_storage_mechanics", examples)]
fn catalog_files_name_no_storage_module() {
    let root = src().join("operations").join("catalog");
    let mut files = rust_files(&root);
    files.push(src().join("operations").join("catalog.rs"));
    let mut findings = Vec::new();
    for path in files.iter().filter(|path| !is_test_file(path, &src())) {
        let source = production_source(&fs::read_to_string(path).unwrap());
        for segments in crate_paths(&source) {
            let names_cache = segments.first().is_some_and(|first| first == "cache");
            if let Some(module) = segments.iter().find(|segment| {
                STORAGE_MODULES.contains(&segment.as_str())
                    || (names_cache && CACHE_STORAGE_MODULES.contains(&segment.as_str()))
            }) {
                findings.push(format!(
                    "{} names crate::{}: {module}",
                    path.display(),
                    segments.join("::")
                ));
            }
        }
        if source.contains("std::fs") {
            findings.push(format!("{} names std::fs", path.display()));
        }
        for word in words(&source).filter(|word| STORAGE_CRATES.contains(word)) {
            findings.push(format!("{} names {word}", path.display()));
        }
    }
    assert!(files.len() > 20, "the catalog directory was not found");
    assert!(
        findings.is_empty(),
        "catalog operations name storage mechanics:\n{}",
        findings.join("\n")
    );
}

/// Implementation aid with no Rule binding: it checks that the path scan reads
/// braced groups and drops test items, so the bound test cannot pass by accident.
#[test]
fn the_scan_finds_a_storage_module_in_a_braced_use() {
    let source = "use crate::{cache::{read, connection::open}, jsonl};\nfn f() { crate::layout::x(); }\n#[cfg(test)]\nmod tests { use crate::shards; }\n";
    let paths = crate_paths(&production_source(source));
    let segments: Vec<&str> = paths.iter().flatten().map(String::as_str).collect();
    assert!(segments.contains(&"connection"));
    assert!(segments.contains(&"jsonl"));
    assert!(segments.contains(&"layout"));
    assert!(!segments.contains(&"shards"));
}

#[test]
#[verifies("rule_repository_handle_stays_a_handle", examples)]
fn repository_impl_blocks_live_in_repository_rs() {
    let home = src().join("repository.rs");
    let mut homes = Vec::new();
    for path in rust_files(&src()) {
        let source = production_source(&fs::read_to_string(&path).unwrap());
        for line in source.lines().map(str::trim) {
            let Some(rest) = line.strip_prefix("impl") else {
                continue;
            };
            let header = rest.split('{').next().unwrap_or("");
            let target = header.split_whitespace().last().unwrap_or("");
            let inherent = !words(header).any(|word| word == "for");
            if inherent && (target == "Repository" || target.ends_with("::Repository")) {
                homes.push(path.clone());
            }
        }
    }
    assert!(
        homes.contains(&home),
        "repository.rs has no impl Repository block"
    );
    homes.retain(|path| path != &home);
    assert!(
        homes.is_empty(),
        "impl Repository blocks outside repository.rs; a use case is a function over the handle: {homes:?}"
    );
}
