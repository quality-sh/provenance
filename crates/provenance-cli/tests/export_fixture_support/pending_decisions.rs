use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The audit fixtures retain pending decisions without treating them as final.
/// This changes only the import input, not the repository graph or its audit.
#[allow(dead_code)]
pub fn keep_decision_links_pending(document: &mut Value) {
    let approved = document["resolutions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|resolution| resolution["status"] == "approved")
        .map(|resolution| resolution["id"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    for question in document["questions"].as_array_mut().unwrap() {
        if question["status"] == "answered"
            && question["resolution_id"]
                .as_str()
                .is_some_and(|id| !approved.contains(id))
        {
            question["status"] = json!("open");
        }
    }
    for rule in document["rules"].as_array_mut().unwrap() {
        if rule["status"] == "active"
            && rule["resolution_ids"].as_array().is_some_and(|ids| {
                ids.iter()
                    .any(|id| !approved.contains(id.as_str().unwrap()))
            })
        {
            rule["status"] = json!("draft");
        }
    }
}
