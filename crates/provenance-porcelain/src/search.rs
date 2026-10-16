//! Scope-wide discovery through the canonical search operation.

use provenance_core::protocol::{GraphNode, QueryResponse, SearchQuery, SearchResult};
use provenance_macros::rule;
use std::{fmt::Display, future::Future, pin::Pin};

/// One future returned by an injected search port.
pub type PortFuture<'a> =
    Pin<Box<dyn Future<Output = Result<QueryResponse<SearchResult>, SearchError>> + Send + 'a>>;

/// A failure from search validation, authorization, or the canonical operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchError {
    InvalidOptions,
    AccessDenied,
    Operation {
        message: String,
        detail: serde_json::Value,
    },
}

impl Display for SearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported search options"),
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
            .map_err(|_| SearchError::InvalidOptions)?;
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
fn lifecycle_suffix(node: &GraphNode) -> &'static str {
    use provenance_core::model::{
        QuestionStatus, RequirementStatus, ResolutionStatus, RuleStatus, TopicStatus,
    };
    match node {
        GraphNode::Requirement(record) => match record.status {
            RequirementStatus::Active => " · active",
            RequirementStatus::Discovery => " · discovery",
            RequirementStatus::Refinement => " · refinement",
            RequirementStatus::Resolved => " · resolved",
        },
        GraphNode::Resolution(record) => match record.status {
            ResolutionStatus::Draft => " · draft",
            ResolutionStatus::Review => " · review",
            ResolutionStatus::Proposed => " · proposed",
            ResolutionStatus::Approved => " · approved",
            ResolutionStatus::Rejected => " · rejected",
            ResolutionStatus::Revised => " · revised",
            ResolutionStatus::Superseded => " · superseded",
            ResolutionStatus::Abandoned => " · abandoned",
        },
        GraphNode::Rule(record) => match record.status {
            RuleStatus::Draft => " · draft",
            RuleStatus::Review => " · review",
            RuleStatus::Active => " · active",
            RuleStatus::Deprecated => " · deprecated",
            RuleStatus::Archived => " · archived",
        },
        GraphNode::Topic(record) => match record.status {
            TopicStatus::Open => " · open",
            TopicStatus::Explored => " · explored",
            TopicStatus::Closed => " · closed",
            TopicStatus::Archived => " · archived",
        },
        GraphNode::Question(record) => match record.status {
            QuestionStatus::Open => " · open",
            QuestionStatus::BlockedOnHuman => " · blocked_on_human",
            QuestionStatus::Answered => " · answered",
            QuestionStatus::Archived => " · archived",
        },
        GraphNode::Source(_) | GraphNode::Domain(_) | GraphNode::Boundary(_) => "",
    }
}
