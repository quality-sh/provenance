use provenance_core::{
    protocol::{Stamp, StampPolicy, Stamped},
    threads::{
        DiscussionConversationResult, DiscussionEntry, DiscussionFact, DiscussionResultPage,
        DiscussionStatus, DiscussionStatusFilter, DiscussionSummary,
    },
    Message, ScopeId, StableId,
};
use provenance_porcelain::{
    action::{Action, ActionError},
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

#[derive(Clone)]
pub struct FixtureDiscussionPort {
    calls: Arc<Mutex<Vec<FixtureCall>>>,
    state: Arc<Mutex<FixtureState>>,
}

struct FixtureState {
    discussions: Vec<FixtureDiscussion>,
}

struct FixtureDiscussion {
    head: DiscussionEntry,
    messages: Vec<Message>,
    opening_excerpt: String,
    excerpt_truncated: bool,
}

impl Default for FixtureDiscussionPort {
    fn default() -> Self {
        Self {
            calls: Arc::default(),
            state: Arc::new(Mutex::new(FixtureState {
                discussions: vec![
                    fixture_discussion("a", "req_a", "Opening text", true),
                    fixture_discussion("b", "req_b", "Another opening", false),
                ],
            })),
        }
    }
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
        let answer = self.list_answer(&input);
        Box::pin(async move { answer })
    }

    fn conversation(
        &self,
        input: ConversationInput,
    ) -> PortFuture<'_, Stamped<DiscussionConversationResult>> {
        self.record(Action::Discussion, &input);
        let answer = self.conversation_answer(&input);
        Box::pin(async move { answer })
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
        let receipt = self.write_reply(input);
        Box::pin(async move { receipt })
    }
}

impl FixtureDiscussionPort {
    fn list_answer(&self, input: &ListInput) -> Result<ListAnswer, ActionError> {
        let state = self.state.lock().unwrap();
        let summaries = state
            .discussions
            .iter()
            .filter(|discussion| selected(discussion, input))
            .map(summary)
            .collect::<Vec<_>>();
        let limit = input.limit.unwrap_or(50);
        let result = page(&summaries, limit, "fixture-list", input.cursor.as_deref())?;
        Ok(ListAnswer {
            scope_id: ScopeId::new("default").unwrap(),
            page: Stamped {
                result,
                stamp: stamp(),
                freshness_error: Some("fixture is stale".into()),
            },
        })
    }

    fn conversation_answer(
        &self,
        input: &ConversationInput,
    ) -> Result<Stamped<DiscussionConversationResult>, ActionError> {
        let state = self.state.lock().unwrap();
        let discussion = state
            .discussions
            .iter()
            .find(|discussion| discussion.head.discussion_id == input.discussion_id)
            .ok_or_else(|| {
                operation_error(
                    "the addressed resource does not exist",
                    "resource_not_found",
                )
            })?;
        let limit = input.limit.unwrap_or(50);
        let cursor_name = format!("fixture-conversation:{}", input.discussion_id.as_str());
        let messages = page(
            &discussion.messages,
            limit,
            &cursor_name,
            input.cursor.as_deref(),
        )?;
        Ok(Stamped {
            result: DiscussionConversationResult {
                head: discussion.head.clone(),
                messages,
            },
            stamp: stamp(),
            freshness_error: None,
        })
    }

    fn write_reply(&self, input: ReplyInput) -> Result<DiscussionEntry, ActionError> {
        let mut state = self.state.lock().unwrap();
        let discussion = state
            .discussions
            .iter_mut()
            .find(|discussion| discussion.head.discussion_id == input.discussion_id)
            .ok_or_else(|| operation_error("Discussion does not exist", "resource_not_found"))?;
        if discussion.head.version != input.expected_version {
            return Err(operation_error(
                "stale Discussion version",
                "discussion_version_conflict",
            ));
        }

        let previous = discussion.head.clone();
        let version = previous.version + 1;
        let message_id = StableId::new(item_id("message", version)).unwrap();
        let message = Message {
            schema_version: previous.schema_version,
            scope_id: previous.scope_id.clone(),
            id: message_id.clone(),
            thread_id: previous.thread_id.clone(),
            role: input.role,
            body: input.body,
            created_at: i64::try_from(version).unwrap(),
            ai_metadata: None,
        };
        let receipt = DiscussionEntry {
            id: StableId::new(item_id("entry", version)).unwrap(),
            version,
            predecessor: Some(previous.id.clone()),
            fact: DiscussionFact::Replied,
            message_id: Some(message_id),
            actor: input.actor,
            request_id: input.request_id,
            intent_digest: "sha256:intent".into(),
            ..previous
        };
        discussion.messages.push(message);
        discussion.head = receipt.clone();
        Ok(receipt)
    }
}

fn selected(discussion: &FixtureDiscussion, input: &ListInput) -> bool {
    let parent_matches = input
        .parent
        .as_ref()
        .is_none_or(|parent| parent == &discussion.head.parent);
    let status_matches = match input.status {
        DiscussionStatusFilter::Active => discussion.head.status == DiscussionStatus::Active,
        DiscussionStatusFilter::Resolved => discussion.head.status == DiscussionStatus::Resolved,
        DiscussionStatusFilter::All => true,
    };
    parent_matches && status_matches
}

fn summary(discussion: &FixtureDiscussion) -> DiscussionSummary {
    DiscussionSummary {
        discussion_id: discussion.head.discussion_id.clone(),
        parent: discussion.head.parent.clone(),
        status: discussion.head.status,
        version: discussion.head.version,
        opening_excerpt: discussion.opening_excerpt.clone(),
        excerpt_truncated: discussion.excerpt_truncated,
    }
}

fn page<T: Clone>(
    entries: &[T],
    limit: usize,
    cursor_name: &str,
    cursor: Option<&str>,
) -> Result<DiscussionResultPage<T>, ActionError> {
    let offset = cursor.map_or(Ok(0), |cursor| cursor_offset(cursor_name, limit, cursor))?;
    let end = offset.saturating_add(limit).min(entries.len());
    let has_more = end < entries.len();
    Ok(DiscussionResultPage {
        entries: entries.get(offset..end).unwrap_or_default().to_vec(),
        limit,
        has_more,
        next_cursor: has_more.then(|| format!("{cursor_name}:{limit}:{end}")),
    })
}

fn cursor_offset(name: &str, limit: usize, cursor: &str) -> Result<usize, ActionError> {
    let invalid = || operation_error("invalid cursor; restart the query", "cursor_invalid");
    let fields = cursor
        .strip_prefix(&format!("{name}:"))
        .ok_or_else(invalid)?
        .split_once(':')
        .ok_or_else(invalid)?;
    if fields.0.parse::<usize>().ok() != Some(limit) {
        return Err(invalid());
    }
    fields.1.parse().map_err(|_| invalid())
}

fn operation_error(message: &str, kind: &str) -> ActionError {
    ActionError::OperationDetail {
        message: message.into(),
        detail: json!({"kind":kind}),
    }
}

fn item_id(prefix: &str, version: u64) -> String {
    u8::try_from(version - 1)
        .ok()
        .and_then(|offset| b'a'.checked_add(offset))
        .filter(u8::is_ascii_lowercase)
        .map_or_else(
            || format!("{prefix}_{version}"),
            |suffix| format!("{prefix}_{}", char::from(suffix)),
        )
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

fn fixture_discussion(
    suffix: &str,
    parent_id: &str,
    body: &str,
    excerpt_truncated: bool,
) -> FixtureDiscussion {
    let entry_id = format!("entry_{suffix}");
    let discussion_id = format!("discussion_{suffix}");
    let message_id = format!("message_{suffix}");
    let thread_id = format!("thread_{suffix}");
    let request_id = if suffix == "a" {
        "request_start".to_owned()
    } else {
        format!("request_start_{suffix}")
    };
    let head = serde_json::from_value(json!({
        "schema_version":2, "scope_id":"default", "id":entry_id,
        "parent":{"node_type":"requirement","node_id":parent_id},
        "thread_id":thread_id, "discussion_id":discussion_id,
        "root_message_id":message_id, "version":1,
        "predecessor":null, "status":"active", "fact":"started",
        "message_id":message_id, "actor":"ben", "request_id":request_id,
        "intent_digest":"sha256:intent"
    }))
    .unwrap();
    let message = serde_json::from_value(json!({
        "schema_version":2, "scope_id":"default", "id":message_id,
        "thread_id":thread_id, "role":"user", "body":body, "created_at":1
    }))
    .unwrap();
    FixtureDiscussion {
        head,
        messages: vec![message],
        opening_excerpt: body.into(),
        excerpt_truncated,
    }
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
