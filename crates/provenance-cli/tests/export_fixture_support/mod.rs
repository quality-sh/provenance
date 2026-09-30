use provenance_core::{review::REVIEW_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION};
use std::collections::BTreeSet;
use std::path::Path;

/// Converts a temporary CLI-authored scope into a plain legacy export fixture.
///
/// Native writers now capture review history. Scope export cannot carry that
/// history yet, so tests for the legacy export contract must remove it first.
pub fn make_default_scope_portable(repo: impl AsRef<Path>) {
    let state = repo.as_ref().join(".provenance/state");
    remove_review_proposals(&state.join("scopes/default/ideation"));
    let review = state.join("scopes/default/review");
    if review.exists() {
        std::fs::remove_dir_all(review).unwrap();
    }
    rewrite_manifest(&state.join("manifest.json"));
    rewrite_records(&state.join("scopes/default"));
}

fn remove_review_proposals(ideation: &Path) {
    let proposals = ideation.join("proposal_cards.jsonl");
    let removed = filter_jsonl(&proposals, |record| {
        record["proposal_type"] != "record_revision"
    });
    if removed.is_empty() {
        return;
    }
    for name in ["assertions.jsonl", "dispositions.jsonl"] {
        filter_jsonl(&ideation.join(name), |record| {
            record["proposal_id"]
                .as_str()
                .is_none_or(|id| !removed.contains(id))
        });
    }
}

fn filter_jsonl(path: &Path, keep: impl Fn(&serde_json::Value) -> bool) -> BTreeSet<String> {
    if !path.exists() {
        return BTreeSet::new();
    }
    let mut retained = String::new();
    let mut removed = BTreeSet::new();
    for line in std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        if keep(&record) {
            retained.push_str(line);
            retained.push('\n');
        } else if let Some(id) = record["id"].as_str() {
            removed.insert(id.to_owned());
        }
    }
    std::fs::write(path, retained).unwrap();
    removed
}

fn rewrite_manifest(path: &Path) {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    manifest["schema_version"] = SUPPORTED_SCHEMA_VERSION.0.into();
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn rewrite_records(directory: &Path) {
    if !directory.is_dir() {
        return;
    }
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rewrite_records(&path);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            rewrite_jsonl(&path);
        }
    }
}

fn rewrite_jsonl(path: &Path) {
    let contents = std::fs::read_to_string(path).unwrap();
    let mut rewritten = String::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
        if record["schema_version"] == REVIEW_SCHEMA_VERSION.0 {
            record["schema_version"] = SUPPORTED_SCHEMA_VERSION.0.into();
        }
        rewritten.push_str(&serde_json::to_string(&record).unwrap());
        rewritten.push('\n');
    }
    std::fs::write(path, rewritten).unwrap();
}
