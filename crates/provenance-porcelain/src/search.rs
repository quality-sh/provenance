//! Scope-wide discovery through the canonical search operation.

use provenance_core::model::{
    QuestionStatus, RequirementStatus, ResolutionStatus, RuleStatus, TopicStatus,
};
use provenance_core::protocol::{GraphNode, QueryResponse, SearchQuery, SearchResult};
use provenance_macros::rule;
use std::{fmt::Display, future::Future, pin::Pin};

/// One future returned by an injected search port.
pub type PortFuture<'a> =
    Pin<Box<dyn Future<Output = Result<QueryResponse<SearchResult>, SearchError>> + Send + 'a>>;

/// A failure from search validation, authorization, or the canonical operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchError {
    InvalidOptions {
        reason: String,
    },
    AccessDenied,
    Operation {
        message: String,
        detail: serde_json::Value,
    },
}

impl Display for SearchError {
    #[provenance_macros::rule("rule_search_refusal_names_query")]
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions { reason } => write!(formatter, "{reason}; run `provenance search --text <query>` or `provenance search --kind <kind>`; MCP JSON: {{\"text\": \"<query>\"}} or {{\"node_types\": [\"<kind>\"]}}"),
            Self::AccessDenied => formatter.write_str("search access denied"),
            Self::Operation { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SearchError {}

/// The one canonical operation path needed for scope-wide discovery.
pub trait SearchPort: Send + Sync {
    fn search(&self, request: SearchQuery) -> PortFuture<'_>;
}

impl<P: SearchPort> crate::Porcelain<P> {
    /// Validate one search and invoke its canonical typed operation.
    pub async fn search(
        &self,
        request: SearchQuery,
    ) -> Result<QueryResponse<SearchResult>, SearchError> {
        request
            .validate()
            .map_err(|error| SearchError::InvalidOptions {
                reason: error.to_string(),
            })?;
        self.port.search(request).await
    }
}

/// Render one bounded search page without repeating full record payloads.
#[must_use]
pub fn render_readable(response: &QueryResponse<SearchResult>) -> String {
    let result = &response.result;
    let mut lines = vec![format!("search: {} returned", result.nodes.len())];
    for node in &result.nodes {
        lines.push(format!(
            "- {} {}{}\n  {}",
            node.node_type().as_str(),
            node.id().as_str(),
            lifecycle_suffix(node),
            node.searchable_text().get(1).copied().unwrap_or("")
        ));
    }
    lines.push(format!(
        "bounds: limit={} has_more={} continuation={}",
        result.limit,
        result.has_more,
        result.next_cursor.as_deref().unwrap_or("none")
    ));
    if let Some(error) = &response.freshness_error {
        lines.push(format!("warning: freshness: {error}"));
    }
    lines.join("\n")
}

/// Names the stored lifecycle state in shared readable search output.
#[rule("rule_search_readable_lifecycle")]
const fn lifecycle_suffix(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Requirement(record) => requirement_suffix(&record.status),
        GraphNode::Resolution(record) => resolution_suffix(&record.status),
        GraphNode::Rule(record) => rule_suffix(&record.status),
        GraphNode::Topic(record) => topic_suffix(record.status),
        GraphNode::Question(record) => question_suffix(record.status),
        GraphNode::Source(_) | GraphNode::Domain(_) | GraphNode::Boundary(_) => "",
    }
}

const fn requirement_suffix(status: &RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Active => " · active",
        RequirementStatus::Discovery => " · discovery",
        RequirementStatus::Refinement => " · refinement",
        RequirementStatus::Resolved => " · resolved",
        RequirementStatus::Archived => " · archived",
    }
}

const fn resolution_suffix(status: &ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Draft => " · draft",
        ResolutionStatus::Review => " · review",
        ResolutionStatus::Proposed => " · proposed",
        ResolutionStatus::Approved => " · approved",
        ResolutionStatus::Rejected => " · rejected",
        ResolutionStatus::Revised => " · revised",
        ResolutionStatus::Superseded => " · superseded",
        ResolutionStatus::Abandoned => " · abandoned",
        ResolutionStatus::Archived => " · archived",
    }
}

const fn rule_suffix(status: &RuleStatus) -> &'static str {
    match status {
        RuleStatus::Draft => " · draft",
        RuleStatus::Review => " · review",
        RuleStatus::Active => " · active",
        RuleStatus::Deprecated => " · deprecated",
        RuleStatus::Archived => " · archived",
    }
}

const fn topic_suffix(status: TopicStatus) -> &'static str {
    match status {
        TopicStatus::Open => " · open",
        TopicStatus::Explored => " · explored",
        TopicStatus::Closed => " · closed",
        TopicStatus::Archived => " · archived",
    }
}

const fn question_suffix(status: QuestionStatus) -> &'static str {
    match status {
        QuestionStatus::Open => " · open",
        QuestionStatus::BlockedOnHuman => " · blocked_on_human",
        QuestionStatus::Answered => " · answered",
        QuestionStatus::Archived => " · archived",
    }
}
