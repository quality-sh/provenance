//! Shared Discussion actions and readable results.

use provenance_core::{
    protocol::{Stamp, Stamped},
    threads::{
        DiscussionConversation, DiscussionEntry, DiscussionListPage, DiscussionStatus,
        DiscussionStatusFilter,
    },
    MessageRole, ScopeId, StableId, ThreadParent,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, future::Future, pin::Pin};

/// One explicit Discussion action. Default graph get remains separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscussionAction {
    Discussions,
    Discussion,
    Discuss,
    Reply,
}

impl DiscussionAction {
    pub const ALL: [Self; 4] = [
        Self::Discussions,
        Self::Discussion,
        Self::Discuss,
        Self::Reply,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discussions => "discussions",
            Self::Discussion => "discussion",
            Self::Discuss => "discuss",
            Self::Reply => "reply",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.as_str() == word)
    }
}

/// Input JSON schema for one named Discussion action.
pub fn input_schema(action: DiscussionAction) -> serde_json::Value {
    let schema = match action {
        DiscussionAction::Discussions => schemars::schema_for!(ListInput),
        DiscussionAction::Discussion => schemars::schema_for!(ConversationInput),
        DiscussionAction::Discuss => schemars::schema_for!(StartInput),
        DiscussionAction::Reply => schemars::schema_for!(ReplyInput),
    };
    serde_json::to_value(schema).expect("Discussion input schema is JSON")
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
        result: DiscussionListPage,
        limit: usize,
        has_more: bool,
        stamp: Option<Stamp>,
        freshness_error: Option<String>,
    },
    Conversation {
        result: DiscussionConversation,
        limit: usize,
        has_more: bool,
        stamp: Option<Stamp>,
        freshness_error: Option<String>,
    },
    Written {
        receipt: DiscussionEntry,
    },
}

/// A refusal from validation, host grants, or a canonical operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscussionError {
    InvalidOptions,
    AccessDenied,
    Operation {
        message: String,
        detail: serde_json::Value,
    },
}

impl Display for DiscussionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported Discussion options"),
            Self::AccessDenied => formatter.write_str("Discussion access denied"),
            Self::Operation { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DiscussionError {}

pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DiscussionError>> + Send + 'a>>;

/// A list answer and the scope selected by the trusted host.
#[derive(Clone, Debug)]
pub struct ListAnswer {
    pub scope_id: ScopeId,
    pub page: Stamped<DiscussionListPage>,
}

/// Canonical Discussion operations provided by the bound host.
pub trait DiscussionPort: Send + Sync {
    fn list(&self, input: ListInput) -> PortFuture<'_, ListAnswer>;
    fn conversation(
        &self,
        input: ConversationInput,
    ) -> PortFuture<'_, Stamped<DiscussionConversation>>;
    fn start(&self, input: StartInput) -> PortFuture<'_, DiscussionEntry>;
    fn reply(&self, input: ReplyInput) -> PortFuture<'_, DiscussionEntry>;
}

impl<P: DiscussionPort> crate::Porcelain<P> {
    /// List addressed Discussions after the host selects permitted parent kinds.
    pub async fn discussions(
        &self,
        input: ListInput,
    ) -> Result<DiscussionOutcome, DiscussionError> {
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
            has_more: result.next_cursor.is_some(),
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
    ) -> Result<DiscussionOutcome, DiscussionError> {
        let limit = checked_limit(input.limit)?;
        let Stamped {
            result,
            stamp,
            freshness_error,
        } = self.port.conversation(input).await?;
        Ok(DiscussionOutcome::Conversation {
            has_more: result.messages.next_cursor.is_some(),
            result,
            limit,
            stamp: Some(stamp),
            freshness_error,
        })
    }

    /// Start one Discussion through the canonical publication operation.
    pub async fn discuss(&self, input: StartInput) -> Result<DiscussionOutcome, DiscussionError> {
        Ok(DiscussionOutcome::Written {
            receipt: self.port.start(input).await?,
        })
    }

    /// Reply through the canonical publication operation.
    pub async fn reply(&self, input: ReplyInput) -> Result<DiscussionOutcome, DiscussionError> {
        Ok(DiscussionOutcome::Written {
            receipt: self.port.reply(input).await?,
        })
    }
}

fn checked_limit(limit: Option<usize>) -> Result<usize, DiscussionError> {
    let limit = limit.unwrap_or(50);
    (1..=200)
        .contains(&limit)
        .then_some(limit)
        .ok_or(DiscussionError::InvalidOptions)
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
                status_filter_word(*status),
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
            lines.push(bounds(*limit, *has_more, result.messages.next_cursor.as_deref()));
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

const fn status_filter_word(status: DiscussionStatusFilter) -> &'static str {
    match status {
        DiscussionStatusFilter::Active => "active",
        DiscussionStatusFilter::Resolved => "resolved",
        DiscussionStatusFilter::All => "all",
    }
}
