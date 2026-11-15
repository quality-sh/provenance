//! Edits of the stored state of a test repository that stop a catch-up.

use serde_json::{json, Value};
use std::path::Path;

/// Puts a string where the line of the Requirement `id` in the repository at
/// `root` holds a list of parents, so that the line does not parse.
pub fn give_refines_a_string(root: &Path, id: &str) {
    let path = root.join(".provenance/state/scopes/default/requirements/req.jsonl");
    let lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|text| {
            let mut line: Value = serde_json::from_str(text).unwrap();
            if line["id"] == id {
                line["refines"] = json!("req_parent");
            }
            format!("{line}\n")
        })
        .collect();
    std::fs::write(&path, lines.concat()).unwrap();
}
