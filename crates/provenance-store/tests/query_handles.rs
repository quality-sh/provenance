//! No query executor reads a projection table, a canonical shard, the
//! working tree, a run file, or git except through the reader's handles.
//!
//! The handles record what they hand out, so `attested` and `live` on the
//! stamp are derived from what was read and cannot omit a table or a live
//! half. The types guard omission, not excess; this gate guards the other
//! direction by text: an executor that named one of these paths would read
//! something the stamp does not list. The scan walks every module under
//! `src/operations/queries/`, subdirectories included, and skips only the
//! test tree, where the oracle copies read canonical state on purpose.
//!
//! The gate refuses module paths, not call spellings, so a reader cannot
//! be renamed away: `use crate::stale::git as vcs` is refused because
//! every `as` alias and every glob in a `use` line is refused, and the
//! path itself is refused wherever it is spelled.

use std::path::{Path, PathBuf};

/// Module paths that reach state past the handles. The executors may name
/// `crate::state_store::StateStore` as a type and `provenance_scanner`'s
/// site readers, which read nothing; the constructors and the scanners
/// that do read are listed by their own spelling.
const BYPASS_PATHS: &[&str] = &[
    "crate::stale::git",
    "crate::stale::gate",
    "crate::cache",
    "crate::layout",
    "crate::publication",
    "crate::shards",
    "crate::jsonl",
    "provenance_scanner::scan_path",
    "provenance_scanner::scan_file",
    "provenance_scanner::scan_path_with_content",
    "provenance_scanner::walker",
    "StateStore::new",
    "ProvenanceLayout::new",
    "open_cache",
    "list_verification_runs",
    "git::",
];

fn queries_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/operations/queries")
}

/// Every `.rs` file under `dir` except `tests.rs` and the `tests` tree,
/// named by its path relative to `dir`.
fn executor_modules(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, modules: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "tests" {
                    walk(root, &path, modules);
                }
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") || name == "tests.rs" {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            modules.push((relative, std::fs::read_to_string(&path).unwrap()));
        }
    }
    let mut modules = Vec::new();
    walk(dir, dir, &mut modules);
    modules.sort();
    modules
}

/// A `use` line that renames or globs an import; the path it names would
/// otherwise escape the path scan.
fn is_renaming_use(line: &str) -> bool {
    let trimmed = line.trim_start();
    (trimmed.starts_with("use ") || trimmed.starts_with("pub use "))
        && (trimmed.contains(" as ") || trimmed.contains('*'))
}

fn bypasses(name: &str, source: &str) -> Vec<String> {
    let mut found: Vec<String> = BYPASS_PATHS
        .iter()
        .filter(|path| source.contains(**path))
        .map(|path| format!("{name}: {path}"))
        .collect();
    found.extend(
        source
            .lines()
            .filter(|line| is_renaming_use(line))
            .map(|line| format!("{name}: {}", line.trim())),
    );
    found
}

#[test]
fn the_gate_sees_a_planted_bypass() {
    let planted = "fn read() { let scans = provenance_scanner::scan_path(repo)?; }";
    assert_eq!(
        bypasses("planted.rs", planted),
        ["planted.rs: provenance_scanner::scan_path"]
    );
}

#[test]
fn the_gate_sees_an_aliased_import() {
    let planted = "use crate::stale::git as vcs;\nfn read() { vcs::changed_files(repo, a, b) }";
    assert_eq!(
        bypasses("planted.rs", planted),
        [
            "planted.rs: crate::stale::git",
            "planted.rs: use crate::stale::git as vcs;"
        ]
    );
    let renamed_type =
        "use crate::state_store::StateStore as Store;\nfn read() { Store::new(layout) }";
    assert_eq!(
        bypasses("planted.rs", renamed_type),
        ["planted.rs: use crate::state_store::StateStore as Store;"]
    );
    let glob = "use provenance_scanner::*;\nfn read() { scan_path(repo) }";
    assert_eq!(
        bypasses("planted.rs", glob),
        ["planted.rs: use provenance_scanner::*;"]
    );
}

#[test]
fn the_gate_walks_subdirectory_modules_and_skips_the_test_tree() {
    let dir = tempfile::tempdir().unwrap();
    let queries = dir.path();
    std::fs::write(queries.join("impact.rs"), "mod scan;\n").unwrap();
    std::fs::create_dir_all(queries.join("impact")).unwrap();
    std::fs::write(
        queries.join("impact/scan.rs"),
        "fn read() { provenance_scanner::scan_path(repo) }\n",
    )
    .unwrap();
    std::fs::create_dir_all(queries.join("tests/oracle")).unwrap();
    std::fs::write(
        queries.join("tests/oracle/impact.rs"),
        "fn read() { provenance_scanner::scan_path(repo) }\n",
    )
    .unwrap();
    std::fs::write(queries.join("tests.rs"), "mod oracle;\n").unwrap();
    let names: Vec<String> = executor_modules(queries)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["impact.rs", "impact/scan.rs"]);
}

#[test]
fn no_query_module_bypasses_the_handles() {
    let modules = executor_modules(&queries_dir());
    assert!(
        modules.len() >= 7,
        "the scan must see the executor modules; saw {modules:?}"
    );
    let found: Vec<String> = modules
        .iter()
        .flat_map(|(name, source)| bypasses(name, source))
        .collect();
    assert!(
        found.is_empty(),
        "query modules reading past the handles: {found:?}"
    );
}
