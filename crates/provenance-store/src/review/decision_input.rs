use provenance_core::{
    CanonicalArtifact, DispositionActor, DispositionDecision, IdeationEvidenceReference,
    MessageRole, ScopeId, StableId,
};
use serde::{Deserialize, Serialize};

/// Submits the record's current review revision as an immutable `proposed`
/// candidate. The store derives the binding; the caller never states it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitRequirementReview {
    pub scope_id: ScopeId,
    pub request_id: StableId,
    pub actor: String,
    pub requirement_id: StableId,
    /// The owning agent of the record, checked like an edit's `declared_by`.
    pub declared_by: Option<String>,
    pub proposal_id: StableId,
    pub proposal_key: String,
    pub title: String,
    pub summary: String,
    pub confidence: Option<f64>,
    pub source_ids: Vec<StableId>,
    pub evidence_references: Vec<IdeationEvidenceReference>,
    /// Assertion lineage. A resubmission's rejection link lives on the cycle,
    /// never here.
    pub builds_on: Vec<provenance_core::AssertionId>,
    /// Refuses when the record moved on since the caller read it.
    pub expected_revision: Option<StableId>,
    /// The rejected proposal this submission revises. The store checks the
    /// rejection and records both links.
    pub revises: Option<StableId>,
}

/// Records one guarded Disposition on a review submission, with optional
/// feedback published in the same commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideRequirementReview {
    pub scope_id: ScopeId,
    pub request_id: StableId,
    pub actor: DispositionActor,
    pub proposal_id: StableId,
    pub disposition_id: StableId,
    pub decision: DispositionDecision,
    /// Never empty. Rejection keeps this with feedback absent.
    pub rationale: String,
    /// The human existing-artifact exception: a person accepting names the
    /// ratified artifact instead of an assertion.
    pub canonical_artifact: Option<CanonicalArtifact>,
    pub feedback: Option<ReviewFeedback>,
    /// The owning agent of the record under review, checked when feedback
    /// opens a Discussion on it.
    pub declared_by: Option<String>,
}

/// A reviewer comment published atomically with its decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFeedback {
    pub role: MessageRole,
    pub body: String,
}

/// Withdraws a pending submission from review. The candidate, its feedback,
/// and the graph record all stay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawRequirementReview {
    pub scope_id: ScopeId,
    pub request_id: StableId,
    pub actor: String,
    pub proposal_id: StableId,
    pub declared_by: Option<String>,
    /// Optional nonempty reason, kept as part of the withdrawal history.
    pub reason: Option<String>,
}
