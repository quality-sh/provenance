use provenance_core::{
    protocol::{Stamp, StampPolicy, Stamped},
    threads::{DiscussionConversationResult, DiscussionEntry, DiscussionResultPage},
    ScopeId,
};
use provenance_porcelain::{
    action::Action,
    discussion::{
        ConversationInput, DiscussionPort, ListAnswer, ListInput, PortFuture, ReplyInput,
        StartInput,
    },
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct FixtureCall {
    pub action: Action,
    pub input: Value,
}

#[derive(Clone, Default)]
pub struct FixtureDiscussionPort {
    calls: Arc<Mutex<Vec<FixtureCall>>>,
}

impl FixtureDiscussionPort {
    pub fn calls(&self) -> Vec<FixtureCall> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, action: Action, input: &impl Serialize) {
        self.calls.lock().unwrap().push(FixtureCall {
            action,
            input: serde_json::to_value(input).unwrap(),
        });
    }
}

impl DiscussionPort for FixtureDiscussionPort {
    fn list(&self, input: ListInput) -> PortFuture<'_, ListAnswer> {
        self.record(Action::Discussions, &input);
        Box::pin(async {
            Ok(ListAnswer {
                scope_id: ScopeId::new("default").unwrap(),
                page: Stamped {
                    result: list_page(),
                    stamp: stamp(),
                    freshness_error: Some("fixture is stale".into()),
                },
            })
        })
    }

    fn conversation(
        &self,
        input: ConversationInput,
    ) -> PortFuture<'_, Stamped<DiscussionConversationResult>> {
        self.record(Action::Discussion, &input);
        Box::pin(async {
            Ok(Stamped {
                result: conversation(),
                stamp: stamp(),
                freshness_error: None,
            })
        })
    }

    fn start(&self, input: StartInput) -> PortFuture<'_, DiscussionEntry> {
        self.record(Action::Discuss, &input);
        let receipt = entry(
            "entry_a",
            "discussion_a",
            "message_a",
            input.request_id.as_str(),
            1,
            "started",
        );
        Box::pin(async move { Ok(receipt) })
    }

    fn reply(&self, input: ReplyInput) -> PortFuture<'_, DiscussionEntry> {
        self.record(Action::Reply, &input);
        let receipt = entry(
            "entry_b",
            input.discussion_id.as_str(),
            "message_b",
            input.request_id.as_str(),
            2,
            "replied",
        );
        Box::pin(async move { Ok(receipt) })
    }
}

fn stamp() -> Stamp {
    Stamp {
        serial: 7,
        digest: "sha256:fixture".into(),
        instance_id: "fixture".into(),
        derivation: 1,
        policy: StampPolicy::CatchUpFailed,
        attested: vec!["discussions".into()],
        live: Vec::new(),
    }
}

fn list_page() -> DiscussionResultPage<provenance_core::threads::DiscussionSummary> {
    serde_json::from_value(json!({
        "entries": [{
            "discussion_id":"discussion_a",
            "parent":{"node_type":"requirement","node_id":"req_a"},
            "status":"active", "version":1,
            "opening_excerpt":"Opening text", "excerpt_truncated":true
        }],
        "limit":1, "has_more":true, "next_cursor":"next-page"
    }))
    .unwrap()
}

fn conversation() -> DiscussionConversationResult {
    serde_json::from_value(json!({
        "head": entry_value(
            "entry_a", "discussion_a", "message_a", "request_start", 1, "started"
        ),
        "messages": {
            "entries": [{
                "schema_version":2, "scope_id":"default", "id":"message_a",
                "thread_id":"thread_a", "role":"user", "body":"Opening text",
                "created_at":1
            }],
            "limit":1, "has_more":true, "next_cursor":"next-message"
        }
    }))
    .unwrap()
}

fn entry(
    id: &str,
    discussion_id: &str,
    message_id: &str,
    request_id: &str,
    version: u64,
    fact: &str,
) -> DiscussionEntry {
    serde_json::from_value(entry_value(
        id,
        discussion_id,
        message_id,
        request_id,
        version,
        fact,
    ))
    .unwrap()
}

fn entry_value(
    id: &str,
    discussion_id: &str,
    message_id: &str,
    request_id: &str,
    version: u64,
    fact: &str,
) -> Value {
    json!({
        "schema_version":2, "scope_id":"default", "id":id,
        "parent":{"node_type":"requirement","node_id":"req_a"},
        "thread_id":"thread_a", "discussion_id":discussion_id,
        "root_message_id":"message_a", "version":version,
        "predecessor":null, "status":"active", "fact":fact,
        "message_id":message_id, "actor":"ben", "request_id":request_id,
        "intent_digest":"sha256:intent"
    })
}
