#![allow(dead_code)]
#[path = "review_support/mod.rs"]
mod base;
pub use base::*;
use provenance_core::threads::{Discussion, DiscussionMessagesQuery, DiscussionSelector};
use provenance_store::{review::WriteDiscussion, state_store::StateStore};
use serde_json::{json, Value};

pub fn write(action: Value) -> WriteDiscussion {
    let mut input = json!({
        "scope_id": "default",
        "parent": {"node_type": "requirement", "node_id": "req_a"},
        "actor": "ben",
        "declared_by": null
    });
    input["action"] = action;
    serde_json::from_value(input).unwrap()
}
pub fn start(store: &StateStore, body: &str) -> Discussion {
    store
        .write_discussion(write(json!({"kind":"start","role":"user","body":body})))
        .unwrap()
}
pub fn reply(discussion: &Discussion, body: &str) -> WriteDiscussion {
    write(
        json!({"kind":"reply","discussion_id":discussion.discussion_id,"expected_version":discussion.version,"role":"user","body":body}),
    )
}
pub fn status(discussion: &Discussion, status: &str) -> WriteDiscussion {
    write(
        json!({"kind":"set_status","discussion_id":discussion.discussion_id,"expected_version":discussion.version,"status":status}),
    )
}
/// A Discussion with five Messages: three pages at a limit of two.
pub fn five_messages(store: &StateStore) -> Discussion {
    let mut latest = start(store, "a");
    for n in 0..4 {
        latest = store
            .write_discussion(reply(&latest, &format!("reply_{n}")))
            .unwrap();
    }
    latest
}
/// The query for `limit` Messages of `latest`, from `cursor` or from the start.
pub fn messages_query(
    latest: &Discussion,
    limit: usize,
    cursor: Option<String>,
) -> DiscussionMessagesQuery {
    DiscussionMessagesQuery {
        parent: latest.parent.clone(),
        selector: DiscussionSelector::Discussion {
            discussion_id: latest.discussion_id.clone(),
        },
        limit,
        cursor,
    }
}
