//! An edit of the stored state of a test repository that gives two records
//! that do not agree, so that a catch-up stops with no file and no line.

use serde_json::json;
use std::path::Path;

/// Appends two Requirement lines to the repository at `root`, `req_c` and
/// `req_d`, that depend on each other.
pub fn add_a_depends_on_cycle(root: &Path) {
    let path = root.join(".provenance/state/scopes/default/requirements/req.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    for (id, depends_on) in [("req_c", "req_d"), ("req_d", "req_c")] {
        let line = json!({
            "scope_id": "default", "id": id, "statement": "The record is readable.",
            "status": "discovery", "depends_on": [depends_on], "supersedes": []
        });
        text = format!("{text}{line}\n");
    }
    std::fs::write(&path, text).unwrap();
}
