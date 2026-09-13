#![allow(dead_code)]
#[path = "review_support/mod.rs"]
mod base;
pub use base::*;
use provenance_core::threads::DiscussionEntry;
use provenance_store::{review::WriteDiscussion, state_store::StateStore};
use serde_json::{json, Value};

pub fn write(request: &str, action: Value) -> WriteDiscussion {
    let mut input = json!({
        "scope_id": "default",
        "parent": {"node_type": "requirement", "node_id": "req_a"},
        "request_id": request,
        "actor": "ben",
        "declared_by": null
    });
    input["action"] = action;
    serde_json::from_value(input).unwrap()
}
pub fn start(store: &StateStore, request: &str) -> DiscussionEntry {
    store
        .write_discussion(write(
            request,
            json!({"kind":"start","role":"user","body":request}),
        ))
        .unwrap()
}
pub fn reply(entry: &DiscussionEntry, request: &str) -> WriteDiscussion {
    write(
        request,
        json!({"kind":"reply","discussion_id":entry.discussion_id,"expected_version":entry.version,"role":"user","body":request}),
    )
}
pub fn status(entry: &DiscussionEntry, request: &str, status: &str) -> WriteDiscussion {
    write(
        request,
        json!({"kind":"set_status","discussion_id":entry.discussion_id,"expected_version":entry.version,"status":status}),
    )
}
