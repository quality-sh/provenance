//! Shared Discussion actions and readable results.

use crate::action::{Action, ActionError};
use provenance_core::{
    protocol::{Stamp, Stamped},
    threads::{
        DiscussionConversationResult, DiscussionEntry, DiscussionResultPage, DiscussionStatus,
        DiscussionStatusFilter, DiscussionSummary,
    },
    MessageRole, ScopeId, StableId, ThreadParent,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin};

/// Input JSON schema for one named Discussion action.
pub fn input_schema(action: Action) -> serde_json::Value {
    let schema = match action {
        Action::Discussions => schemars::schema_for!(ListInput),
        Action::Discussion => schemars::schema_for!(ConversationInput),
        Action::Discuss => schemars::schema_for!(StartInput),
        Action::Reply => schemars::schema_for!(ReplyInput),
        _ => unreachable!("record actions use registered operation schemas"),
    };
    serde_json::to_value(schema).expect("Discussion input schema is JSON")
}

/// The target field names the record parent or the independent Discussion ID.
pub fn target_field(action: Action) -> &'static str {
    match action {
        Action::Discussions | Action::Discuss => "parent",
        Action::Discussion | Action::Reply => "discussion_id",
        _ => unreachable!("record actions use registered target bindings"),
    }
}

/// The shared typed result schema.
pub fn output_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(DiscussionOutcome))
        .expect("Discussion result schema is JSON")
}

/// A bounded list selected by a scope or one parent.
#[derive(Clone, Debug, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
    pub parent: Option<ThreadParent>,
    #[serde(default)]
    pub status: DiscussionStatusFilter,
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// A bounded conversation selected by its Discussion ID.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConversationInput {
    pub discussion_id: StableId,
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// An independent Discussion start under one graph record.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartInput {
    pub parent: ThreadParent,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
    pub role: MessageRole,
    pub body: String,
}

/// A versioned reply to one addressed Discussion.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplyInput {
    pub discussion_id: StableId,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
    pub expected_version: u64,
    pub role: MessageRole,
    pub body: String,
}

/// The shared structured result for CLI and MCP.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiscussionOutcome {
    List {
        scope_id: ScopeId,
        parent: Option<ThreadParent>,
        status: DiscussionStatusFilter,
        result: DiscussionResultPage<DiscussionSummary>,
        limit: usize,
        has_more: bool,
        stamp: Option<Stamp>,
        freshness_error: Option<String>,
    },
    Conversation {
        result: Box<DiscussionConversationResult>,
        limit: usize,
        has_more: bool,
        stamp: Option<Stamp>,
        freshness_error: Option<String>,
    },
    Written {
        receipt: DiscussionEntry,
    },
}

pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ActionError>> + Send + 'a>>;

/// A list answer and the scope selected by the trusted host.
#[derive(Clone, Debug)]
pub struct ListAnswer {
    pub scope_id: ScopeId,
    pub page: Stamped<DiscussionResultPage<DiscussionSummary>>,
}

/// Canonical Discussion operations provided by the bound host.
pub trait DiscussionPort: Send + Sync {
    fn list(&self, input: ListInput) -> PortFuture<'_, ListAnswer>;
    fn conversation(
        &self,
        input: ConversationInput,
    ) -> PortFuture<'_, Stamped<DiscussionConversationResult>>;
    fn start(&self, input: StartInput) -> PortFuture<'_, DiscussionEntry>;
    fn reply(&self, input: ReplyInput) -> PortFuture<'_, DiscussionEntry>;
}

impl<P: DiscussionPort> crate::Porcelain<P> {
    /// Parse and run one declared Discussion action for any host consumer.
    pub async fn execute_discussion(
        &self,
        action: Action,
        arguments: Value,
    ) -> Result<DiscussionOutcome, ActionError> {
        match action {
            Action::Discussions => self.discussions(parse_input(arguments)?).await,
            Action::Discussion => self.conversation(parse_input(arguments)?).await,
            Action::Discuss => self.discuss(parse_input(arguments)?).await,
            Action::Reply => self.reply(parse_input(arguments)?).await,
            _ => Err(ActionError::InvalidOptions),
        }
    }

    /// List addressed Discussions after the host selects permitted parent kinds.
    pub async fn discussions(&self, input: ListInput) -> Result<DiscussionOutcome, ActionError> {
        let limit = checked_limit(input.limit)?;
        let parent = input.parent.clone();
        let status = input.status;
        let ListAnswer {
            scope_id,
            page:
                Stamped {
                    result,
                    stamp,
                    freshness_error,
                },
        } = self.port.list(input).await?;
        Ok(DiscussionOutcome::List {
            scope_id,
            parent,
            status,
            has_more: result.has_more,
            result,
            limit,
            stamp: Some(stamp),
            freshness_error,
        })
    }

    /// Read one addressed Discussion and a bounded Message page.
    pub async fn conversation(
        &self,
        input: ConversationInput,
    ) -> Result<DiscussionOutcome, ActionError> {
        let limit = checked_limit(input.limit)?;
        let Stamped {
            result,
            stamp,
            freshness_error,
        } = self.port.conversation(input).await?;
        Ok(DiscussionOutcome::Conversation {
            has_more: result.messages.has_more,
            result: Box::new(result),
            limit,
            stamp: Some(stamp),
            freshness_error,
        })
    }

    /// Start one Discussion through the canonical publication operation.
    pub async fn discuss(&self, input: StartInput) -> Result<DiscussionOutcome, ActionError> {
        Ok(DiscussionOutcome::Written {
            receipt: self.port.start(input).await?,
        })
    }

    /// Reply through the canonical publication operation.
    pub async fn reply(&self, input: ReplyInput) -> Result<DiscussionOutcome, ActionError> {
        Ok(DiscussionOutcome::Written {
            receipt: self.port.reply(input).await?,
        })
    }
}

fn parse_input<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, ActionError> {
    serde_json::from_value(value).map_err(|_| ActionError::InvalidOptions)
}

fn checked_limit(limit: Option<usize>) -> Result<usize, ActionError> {
    let limit = limit.unwrap_or(50);
    (1..=200)
        .contains(&limit)
        .then_some(limit)
        .ok_or(ActionError::InvalidOptions)
}

/// Render the fields that identify a Discussion and the bounds of its page.
pub fn render_readable(outcome: &DiscussionOutcome) -> String {
    match outcome {
        DiscussionOutcome::List {
            scope_id,
            parent,
            status,
            result,
            limit,
            has_more,
            freshness_error,
            ..
        } => {
            let selector = parent.as_ref().map_or_else(
                || "scope".to_owned(),
                |parent| format!("{} {}", parent.node_type.as_str(), parent.node_id.as_str()),
            );
            let mut lines = vec![format!(
                "discussions scope={} parent={} status={}: {} returned",
                scope_id.as_str(),
                selector,
                status.as_str(),
                result.entries.len()
            )];
            for entry in &result.entries {
                lines.push(format!(
                    "- {} parent={} {} status={} version={} truncated={}\n  {}",
                    entry.discussion_id.as_str(),
                    entry.parent.node_type.as_str(),
                    entry.parent.node_id.as_str(),
                    status_word(entry.status),
                    entry.version,
                    entry.excerpt_truncated,
                    entry.opening_excerpt
                ));
            }
            lines.push(bounds(*limit, *has_more, result.next_cursor.as_deref()));
            if let Some(error) = freshness_error {
                lines.push(format!("warning: freshness: {error}"));
            }
            lines.join("\n")
        }
        DiscussionOutcome::Conversation {
            result,
            limit,
            has_more,
            freshness_error,
            ..
        } => {
            let head = &result.head;
            let mut lines =
                vec![format!(
                "discussion {} parent={} {} status={} version={} thread={} request={} digest={}",
                head.discussion_id.as_str(), head.parent.node_type.as_str(),
                head.parent.node_id.as_str(), status_word(head.status), head.version,
                head.thread_id.as_str(), head.request_id.as_str(), head.intent_digest
            )];
            for message in &result.messages.entries {
                lines.push(format!(
                    "- message {} role={:?}\n  {}",
                    message.id.as_str(),
                    message.role,
                    message.body
                ));
            }
            lines.push(bounds(
                *limit,
                *has_more,
                result.messages.next_cursor.as_deref(),
            ));
            if let Some(error) = freshness_error {
                lines.push(format!("warning: freshness: {error}"));
            }
            lines.join("\n")
        }
        DiscussionOutcome::Written { receipt } => format!(
            "discussion {} parent={} {} status={} version={} message={} request={} digest={}",
            receipt.discussion_id.as_str(),
            receipt.parent.node_type.as_str(),
            receipt.parent.node_id.as_str(),
            status_word(receipt.status),
            receipt.version,
            receipt.message_id.as_ref().map_or("none", StableId::as_str),
            receipt.request_id.as_str(),
            receipt.intent_digest
        ),
    }
}

fn bounds(limit: usize, has_more: bool, cursor: Option<&str>) -> String {
    format!(
        "bounds: limit={} has_more={} continuation={}",
        limit,
        has_more,
        cursor.unwrap_or("none")
    )
}

const fn status_word(status: DiscussionStatus) -> &'static str {
    match status {
        DiscussionStatus::Active => "active",
        DiscussionStatus::Resolved => "resolved",
    }
}
