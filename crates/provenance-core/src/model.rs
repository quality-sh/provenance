mod artifacts;
mod collaboration;
mod graph;
mod ideation;
mod ids;
mod integrations;
mod manifest;
mod parsing;
pub mod projection_row;
pub mod relations;
mod services;
mod shaping;
mod validation;

pub use artifacts::{
    Requirement, RequirementStatus, Resolution, ResolutionInput, ResolutionInputType,
    ResolutionStatus, Rule, RuleSeverity, RuleStatus, Source, SourceReference, SourceType,
};
pub use collaboration::{Message, MessageRole, Thread, ThreadParent, ThreadStatus};
pub use graph::NodeType;
pub use ideation::contributions::{
    ClaimChallenge, Contribution, MaterialClaim, SuggestedArtifactChange, UncertaintyRating,
    UnsupportedRecommendation,
};
pub use ideation::dispositions::{
    disposition_requires_prior_assertion, validate_disposition_intrinsic, CanonicalArtifact,
    DispositionActor, DispositionRecord, ExternalActionCorrelation,
};
pub use ideation::legacy_audit::is_shipped_legacy_disposition_audit;
pub use ideation::lifecycle::{
    effective_proposal_state, ensure_supported_schema_version, packet_qualifies_proposal,
    validate_assertion_intrinsic, validate_disposition_admissible, validate_ideation_aggregate,
    validate_proposal_intrinsic, Assertion, AssertionId, AssertionRecord, IdeationAggregate,
    LegacyProposalPolicy, SUPPORTED_SCHEMA_VERSION,
};
pub use ideation::proposals::{Proposal, ProposalCard, ProposalTraceability};
pub use ideation::synthesis::{
    ConsensusFinding, ContestedClaim, EvidenceGap, MinorityObjection, RequiredHumanDecision,
    SuggestedArtifact, SynthesisPacket, UnsupportedSpeculation,
};
pub use ideation::{
    ArtifactChangeType, CanonicalArtifactType, ContributionStance, DispositionDecision,
    EvidenceQuality, IdeationEvidenceReference, IdeationEvidenceType, IdeationTarget,
    IdeationTargetType, IdentityType, PromotionState, ProposalType, SpeculationMarker,
    UncertaintyLevel,
};
pub use ids::{SchemaVersion, ScopeId, StableId};
pub use integrations::{
    DeclarationAddress, ImplementationBinding, RequirementReview, VerificationBinding,
    VerificationMethod, VerificationRun, VerificationRunStatus,
};
pub use manifest::{Manifest, RepoPathPrefix, Scope};
pub use projection_row::{ColumnValue, ProjectionRow};
pub use services::Domain;
pub use shaping::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, Question, QuestionStatus, ResolutionMethod,
    Topic, TopicStatus,
};
pub use validation::{
    validate_commit_pin, validate_confidence_score, validate_optional_commit_pin,
    validate_optional_confidence_score, validate_resolution_input_content,
};

#[cfg(test)]
mod tests;
