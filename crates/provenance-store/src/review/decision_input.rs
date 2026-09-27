use provenance_core::{
    CanonicalArtifact, DispositionActor, DispositionDecision, IdeationEvidenceReference,
    MessageRole, ScopeId, StableId,
};
use serde::{Deserialize, Serialize};

impl DecideRequirementReview {
    pub(super) fn ensure_canonical_artifact_type_supported(&self) -> anyhow::Result<()> {
        let Some(artifact) = &self.canonical_artifact else {
            return Ok(());
        };
        crate::write_error::ensure!(
            InvalidUpdate,
            matches!(
                artifact.artifact_type,
                provenance_core::CanonicalArtifactType::Source
                    | provenance_core::CanonicalArtifactType::Requirement
                    | provenance_core::CanonicalArtifactType::Resolution
                    | provenance_core::CanonicalArtifactType::Rule
            ),
            "requirement review decisions support source, requirement, resolution, or rule canonical artifacts"
        );
        Ok(())
    }
}

/// Submits the record's current review revision as an immutable `proposed`
/// candidate. The store derives the binding; the caller never states it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitRequirementReview {
    pub scope_id: ScopeId,
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideRequirementReview {
    pub scope_id: ScopeId,
    pub actor: DispositionActor,
    pub proposal_id: StableId,
    pub decision: DispositionDecision,
    /// Required and nonempty for rejection. Optional for other decisions.
    pub rationale: Option<String>,
    /// The human existing-artifact exception: a person accepting names the
    /// ratified artifact instead of an assertion.
    pub canonical_artifact: Option<CanonicalArtifact>,
    pub feedback: Option<ReviewFeedback>,
    /// The owning agent of the record under review, checked when feedback
    /// opens a Discussion on it.
    pub declared_by: Option<String>,
}

/// A reviewer comment published atomically with its decision.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFeedback {
    pub role: MessageRole,
    pub body: String,
}

/// Withdraws a pending submission from review. The candidate, its feedback,
/// and the graph record all stay.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawRequirementReview {
    pub scope_id: ScopeId,
    pub actor: String,
    pub proposal_id: StableId,
    pub declared_by: Option<String>,
    /// Optional nonempty reason, kept as part of the withdrawal history.
    pub reason: Option<String>,
}
