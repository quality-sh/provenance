#[path = "discussion/fixture.rs"]
mod fixture;

use fixture::FixtureDiscussionPort;
use provenance_core::threads::DiscussionStatusFilter;
use provenance_macros::verifies;
use provenance_porcelain::{
    action::{Action, ActionError},
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
        .execute_discussion(Action::Discussion, json!({"discussion_id":"discussion_a"}))
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
    let next_cursor = result.next_cursor.as_deref().unwrap();
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
        "warning: freshness: fixture is stale",
    ] {
        assert!(
            readable.contains(expected),
            "missing {expected}: {readable}"
        );
    }
    assert!(readable.contains(next_cursor), "{readable}");
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
    assert!(!*has_more);
    assert!(result.messages.next_cursor.is_none());

    let readable = render_readable(&outcome);
    for expected in [
        "discussion_a",
        "message_a",
        "Opening text",
        "continuation=none",
    ] {
        assert!(
            readable.contains(expected),
            "missing {expected}: {readable}"
        );
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

#[tokio::test]
async fn reply_to_a_missing_discussion_returns_the_host_error() {
    let error = reply(
        &Porcelain::new(FixtureDiscussionPort::default()),
        "missing",
        "request_missing",
        1,
        "No target",
    )
    .await
    .unwrap_err();

    assert_operation_kind(error, "resource_not_found");
}

#[tokio::test]
async fn reply_with_a_stale_version_returns_the_host_error() {
    let error = reply(
        &Porcelain::new(FixtureDiscussionPort::default()),
        "discussion_a",
        "request_stale",
        99,
        "Stale reply",
    )
    .await
    .unwrap_err();

    assert_operation_kind(error, "discussion_version_conflict");
}

#[tokio::test]
async fn reply_increments_the_version_and_refuses_the_old_version() {
    let porcelain = Porcelain::new(FixtureDiscussionPort::default());
    let outcome = reply(
        &porcelain,
        "discussion_a",
        "request_reply",
        1,
        "Second message",
    )
    .await
    .unwrap();
    let DiscussionOutcome::Written { receipt } = outcome else {
        panic!("written outcome")
    };
    assert_eq!(receipt.version, 2);

    let error = reply(
        &porcelain,
        "discussion_a",
        "request_repeated",
        1,
        "Repeated message",
    )
    .await
    .unwrap_err();
    assert_operation_kind(error, "discussion_version_conflict");
}

#[tokio::test]
async fn conversation_keeps_reply_messages_in_entry_order() {
    let porcelain = Porcelain::new(FixtureDiscussionPort::default());
    reply(
        &porcelain,
        "discussion_a",
        "request_second",
        1,
        "Second message",
    )
    .await
    .unwrap();
    reply(
        &porcelain,
        "discussion_a",
        "request_third",
        2,
        "Third message",
    )
    .await
    .unwrap();

    let outcome = porcelain
        .execute_discussion(Action::Discussion, json!({"discussion_id":"discussion_a"}))
        .await
        .unwrap();
    let DiscussionOutcome::Conversation { result, .. } = outcome else {
        panic!("conversation outcome")
    };
    let bodies = result
        .messages
        .entries
        .iter()
        .map(|message| message.body.as_str())
        .collect::<Vec<_>>();
    assert_eq!(bodies, ["Opening text", "Second message", "Third message"]);
    assert_eq!(result.head.version, 3);
}

#[tokio::test]
async fn omitted_read_limits_are_50_in_the_outcome_and_result() {
    let porcelain = Porcelain::new(FixtureDiscussionPort::default());
    let list = porcelain
        .execute_discussion(Action::Discussions, json!({}))
        .await
        .unwrap();
    let DiscussionOutcome::List { result, limit, .. } = list else {
        panic!("list outcome")
    };
    assert_eq!(limit, 50);
    assert_eq!(result.limit, 50);

    let conversation = porcelain
        .execute_discussion(Action::Discussion, json!({"discussion_id":"discussion_a"}))
        .await
        .unwrap();
    let DiscussionOutcome::Conversation { result, limit, .. } = conversation else {
        panic!("conversation outcome")
    };
    assert_eq!(limit, 50);
    assert_eq!(result.messages.limit, 50);
}

#[tokio::test]
async fn returned_cursors_continue_list_and_conversation_pages() {
    let porcelain = Porcelain::new(FixtureDiscussionPort::default());
    let first_list = porcelain
        .execute_discussion(Action::Discussions, json!({"limit":1}))
        .await
        .unwrap();
    let DiscussionOutcome::List { result, .. } = first_list else {
        panic!("list outcome")
    };
    assert_eq!(result.entries[0].discussion_id.as_str(), "discussion_a");
    let list_cursor = result.next_cursor.unwrap();
    let second_list = porcelain
        .execute_discussion(
            Action::Discussions,
            json!({"limit":1, "cursor":list_cursor}),
        )
        .await
        .unwrap();
    let DiscussionOutcome::List { result, .. } = second_list else {
        panic!("list outcome")
    };
    assert_eq!(result.entries[0].discussion_id.as_str(), "discussion_b");
    assert!(!result.has_more);
    assert!(result.next_cursor.is_none());

    reply(
        &porcelain,
        "discussion_a",
        "request_second",
        1,
        "Second message",
    )
    .await
    .unwrap();
    let first_conversation = porcelain
        .execute_discussion(
            Action::Discussion,
            json!({"discussion_id":"discussion_a", "limit":1}),
        )
        .await
        .unwrap();
    let DiscussionOutcome::Conversation { result, .. } = first_conversation else {
        panic!("conversation outcome")
    };
    assert_eq!(result.messages.entries[0].body, "Opening text");
    let message_cursor = result.messages.next_cursor.unwrap();
    let second_conversation = porcelain
        .execute_discussion(
            Action::Discussion,
            json!({
                "discussion_id":"discussion_a", "limit":1, "cursor":message_cursor
            }),
        )
        .await
        .unwrap();
    let DiscussionOutcome::Conversation { result, .. } = second_conversation else {
        panic!("conversation outcome")
    };
    assert_eq!(result.messages.entries[0].body, "Second message");
    assert!(!result.messages.has_more);
    assert!(result.messages.next_cursor.is_none());
}

async fn reply(
    porcelain: &Porcelain<FixtureDiscussionPort>,
    discussion_id: &str,
    request_id: &str,
    expected_version: u64,
    body: &str,
) -> Result<DiscussionOutcome, ActionError> {
    porcelain
        .execute_discussion(
            Action::Reply,
            json!({
                "discussion_id":discussion_id, "request_id":request_id,
                "actor":"ben", "expected_version":expected_version,
                "role":"user", "body":body
            }),
        )
        .await
}

fn assert_operation_kind(error: ActionError, expected: &str) {
    let ActionError::OperationDetail { detail, .. } = error else {
        panic!("operation detail")
    };
    assert_eq!(detail["kind"], expected);
}
