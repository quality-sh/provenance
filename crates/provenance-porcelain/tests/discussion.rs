#[path = "discussion/fixture.rs"]
mod fixture;

use fixture::FixtureDiscussionPort;
use provenance_core::threads::DiscussionStatusFilter;
use provenance_macros::verifies;
use provenance_porcelain::{
    action::Action,
    discussion::{render_readable, DiscussionOutcome, ListInput},
    Porcelain,
};
use serde_json::json;

#[test]
fn list_input_defaults_to_active_and_rejects_host_grants() {
    let input: ListInput = serde_json::from_value(json!({})).unwrap();
    assert_eq!(input.status, DiscussionStatusFilter::Active);
    assert!(serde_json::from_value::<ListInput>(json!({
        "allowed_parent_kinds": ["source"]
    }))
    .is_err());
}

#[tokio::test]
#[verifies("rule_porcelain_discussion_targets", examples)]
async fn actions_send_their_resolved_targets_to_the_port() {
    let port = FixtureDiscussionPort::default();
    let porcelain = Porcelain::new(port.clone());

    porcelain
        .execute_discussion(
            Action::Discussions,
            json!({"parent":{"node_type":"requirement","node_id":"req_a"}}),
        )
        .await
        .unwrap();
    porcelain
        .execute_discussion(
            Action::Discussion,
            json!({"discussion_id":"discussion_a"}),
        )
        .await
        .unwrap();
    porcelain
        .execute_discussion(
            Action::Discuss,
            json!({
                "parent":{"node_type":"requirement","node_id":"req_a"},
                "request_id":"request_start", "actor":"ben", "role":"user",
                "body":"Opening text"
            }),
        )
        .await
        .unwrap();
    porcelain
        .execute_discussion(
            Action::Reply,
            json!({
                "discussion_id":"discussion_a", "request_id":"request_reply",
                "actor":"ben", "expected_version":1, "role":"user",
                "body":"Second message"
            }),
        )
        .await
        .unwrap();

    let calls = port.calls();
    assert_eq!(calls[0].action, Action::Discussions);
    assert_eq!(calls[0].input["parent"]["node_id"], "req_a");
    assert_eq!(calls[1].action, Action::Discussion);
    assert_eq!(calls[1].input["discussion_id"], "discussion_a");
    assert_eq!(calls[2].action, Action::Discuss);
    assert_eq!(calls[2].input["parent"]["node_id"], "req_a");
    assert_eq!(calls[3].action, Action::Reply);
    assert_eq!(calls[3].input["discussion_id"], "discussion_a");
}

#[tokio::test]
async fn list_read_keeps_page_metadata_and_renders_entry_fields() {
    let outcome = Porcelain::new(FixtureDiscussionPort::default())
        .execute_discussion(Action::Discussions, json!({"limit":1}))
        .await
        .unwrap();

    let DiscussionOutcome::List {
        scope_id,
        result,
        limit,
        has_more,
        stamp,
        freshness_error,
        ..
    } = &outcome
    else {
        panic!("list outcome")
    };
    assert_eq!(scope_id.as_str(), "default");
    assert_eq!(result.entries[0].discussion_id.as_str(), "discussion_a");
    assert_eq!(*limit, 1);
    assert!(*has_more);
    assert_eq!(stamp.as_ref().unwrap().serial, 7);
    assert_eq!(freshness_error.as_deref(), Some("fixture is stale"));

    let readable = render_readable(&outcome);
    for expected in [
        "discussion_a",
        "req_a",
        "active",
        "version=1",
        "Opening text",
        "truncated=true",
        "limit=1",
        "next-page",
        "warning: freshness: fixture is stale",
    ] {
        assert!(readable.contains(expected), "missing {expected}: {readable}");
    }
}

#[tokio::test]
async fn conversation_read_keeps_bounds_and_renders_messages() {
    let outcome = Porcelain::new(FixtureDiscussionPort::default())
        .execute_discussion(
            Action::Discussion,
            json!({"discussion_id":"discussion_a", "limit":1}),
        )
        .await
        .unwrap();

    let DiscussionOutcome::Conversation {
        result,
        limit,
        has_more,
        ..
    } = &outcome
    else {
        panic!("conversation outcome")
    };
    assert_eq!(result.head.discussion_id.as_str(), "discussion_a");
    assert_eq!(result.messages.entries[0].body, "Opening text");
    assert_eq!(*limit, 1);
    assert!(*has_more);

    let readable = render_readable(&outcome);
    for expected in ["discussion_a", "message_a", "Opening text", "next-message"] {
        assert!(readable.contains(expected), "missing {expected}: {readable}");
    }
}

#[tokio::test]
async fn writes_return_receipts_and_render_the_written_identity() {
    let port = FixtureDiscussionPort::default();
    let porcelain = Porcelain::new(port.clone());
    let started = porcelain
        .execute_discussion(
            Action::Discuss,
            json!({
                "parent":{"node_type":"requirement","node_id":"req_a"},
                "request_id":"request_start", "actor":"ben", "role":"user",
                "body":"Opening text"
            }),
        )
        .await
        .unwrap();
    let replied = porcelain
        .execute_discussion(
            Action::Reply,
            json!({
                "discussion_id":"discussion_a", "request_id":"request_reply",
                "actor":"ben", "expected_version":1, "role":"user",
                "body":"Second message"
            }),
        )
        .await
        .unwrap();

    assert!(render_readable(&started).contains("request=request_start"));
    let readable = render_readable(&replied);
    assert!(readable.contains("discussion discussion_a"), "{readable}");
    assert!(readable.contains("version=2"), "{readable}");
    assert!(readable.contains("message=message_b"), "{readable}");
    assert!(readable.contains("request=request_reply"), "{readable}");
    assert_eq!(port.calls().len(), 2);
}
