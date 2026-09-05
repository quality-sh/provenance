//! No query executor reads a projection table, a canonical shard, the
//! working tree, a run file, or git except through the reader's handles.
//!
//! The handles record what they hand out, so `attested` and `live` on the
//! stamp are derived from what was read and cannot omit a table or a live
//! half. The types guard omission, not excess; this gate guards the other
//! direction by text: an executor that named one of these spellings would
//! read something the stamp does not list. The scan covers every module
//! under `src/operations/queries/` except the test tree, where the oracle
//! copies read canonical state on purpose.

use std::path::{Path, PathBuf};

/// Spellings that reach state past the handles. Each is a `pub` item
/// today, so only this gate keeps them out of the executors.
const BYPASS_SPELLINGS: &[&str] = &[
    "open_cache",
    "StateStore::new",
    "ProvenanceLayout::new",
    "scan_path",
    "scan_file",
    "list_verification_runs",
    "git::",
];

fn queries_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/operations/queries")
}

fn executor_modules() -> Vec<(String, String)> {
    let mut modules = Vec::new();
    for entry in std::fs::read_dir(queries_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == "tests.rs" {
            continue;
        }
        modules.push((name, std::fs::read_to_string(&path).unwrap()));
    }
    modules.sort();
    modules
}

fn bypasses(name: &str, source: &str) -> Vec<String> {
    BYPASS_SPELLINGS
        .iter()
        .filter(|spelling| source.contains(**spelling))
        .map(|spelling| format!("{name}: {spelling}"))
        .collect()
}

#[test]
fn the_gate_sees_a_planted_bypass() {
    let planted = "fn read() { let scans = provenance_scanner::scan_path(repo)?; }";
    assert_eq!(bypasses("planted.rs", planted), ["planted.rs: scan_path"]);
}

#[test]
fn no_query_module_bypasses_the_handles() {
    let modules = executor_modules();
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
