use provenance_core::SUPPORTED_SCHEMA_VERSION;
use std::path::Path;

/// Converts a temporary CLI-authored scope into a plain legacy export fixture.
///
/// Native writers now capture review history. Scope export cannot carry that
/// history yet, so tests for the legacy export contract must remove it first.
pub fn make_default_scope_portable(repo: impl AsRef<Path>) {
    let state = repo.as_ref().join(".provenance/state");
    let review = state.join("scopes/default/review");
    if review.exists() {
        std::fs::remove_dir_all(review).unwrap();
    }
    rewrite_manifest(&state.join("manifest.json"));
    rewrite_records(&state.join("scopes/default"));
}

fn rewrite_manifest(path: &Path) {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    manifest["schema_version"] = SUPPORTED_SCHEMA_VERSION.0.into();
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn rewrite_records(directory: &Path) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rewrite_records(&path);
        } else if path.extension().is_some_and(|extension| extension == "jsonl") {
            rewrite_jsonl(&path);
        }
    }
}

fn rewrite_jsonl(path: &Path) {
    let contents = std::fs::read_to_string(path).unwrap();
    let mut rewritten = String::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
        if record.get("schema_version").is_some() {
            record["schema_version"] = SUPPORTED_SCHEMA_VERSION.0.into();
        }
        rewritten.push_str(&serde_json::to_string(&record).unwrap());
        rewritten.push('\n');
    }
    std::fs::write(path, rewritten).unwrap();
}
