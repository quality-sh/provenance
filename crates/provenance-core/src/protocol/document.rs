use super::GraphNode;
use crate::{DispositionDecision, Message, StableId, Thread};
use serde::{Deserialize, Serialize};

/// Selects one active Requirement branch at a projection revision.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadDocumentQuery {
    pub id: String,
    #[serde(default)]
    pub exclude_terminal: bool,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    #[cfg_attr(feature = "schema", schemars(range(min = 1, max = super::QUERY_MAX_LIMIT)))]
    pub limit: usize,
}
const fn default_limit() -> usize {
    super::QUERY_DEFAULT_LIMIT
}

/// A record's role in this document. References never expand membership.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentEntry {
    Member {
        node: GraphNode,
        review: DocumentReviewSummary,
    },
    Reference {
        node: GraphNode,
        review: DocumentReviewSummary,
    },
    Thread {
        thread: Thread,
    },
    Message {
        message: Message,
    },
}

/// The current review outcome of one record in a document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentReviewOutcome {
    Pending,
    Accepted,
    Rejected,
}

/// The compact review state for one record in a document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentReviewSummary {
    pub outcome: Option<DocumentReviewOutcome>,
    pub pending_proposal_id: Option<StableId>,
    pub comment_count: usize,
}

impl From<&crate::review::RequirementDecisionState> for DocumentReviewSummary {
    fn from(state: &crate::review::RequirementDecisionState) -> Self {
        if let Some(pending) = &state.pending {
            return Self {
                outcome: Some(DocumentReviewOutcome::Pending),
                pending_proposal_id: Some(pending.proposal_id.clone()),
                comment_count: 0,
            };
        }
        if let Some(accepted) = &state.current_acceptance {
            return Self {
                outcome: Some(DocumentReviewOutcome::Accepted),
                pending_proposal_id: None,
                comment_count: usize::from(accepted.feedback_message_id.is_some()),
            };
        }
        let rejection = state.decisions.iter().rev().find(|decision| {
            decision.revision.as_ref() == state.current_revision.as_ref()
                && decision.disposition.decision == DispositionDecision::Rejected
        });
        Self {
            outcome: rejection.map(|_| DocumentReviewOutcome::Rejected),
            pending_proposal_id: None,
            comment_count: rejection
                .map(|decision| usize::from(decision.feedback_message_id.is_some()))
                .unwrap_or(0),
        }
    }
}

/// Review outcome totals for all filtered records in a document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DocumentReviewTotals {
    pub pending: usize,
    pub accepted: usize,
    pub rejected: usize,
}

impl DocumentReviewTotals {
    pub fn include(&mut self, summary: &DocumentReviewSummary) {
        match summary.outcome {
            Some(DocumentReviewOutcome::Pending) => self.pending += 1,
            Some(DocumentReviewOutcome::Accepted) => self.accepted += 1,
            Some(DocumentReviewOutcome::Rejected) => self.rejected += 1,
            None => {}
        }
    }
}

/// One bounded page. Only a terminal sequence establishes completeness.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ReadDocumentResult {
    pub root_id: StableId,
    pub limit: usize,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub review_totals: DocumentReviewTotals,
    pub entries: Vec<DocumentEntry>,
}
