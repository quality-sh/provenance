#[cfg(test)]
use provenance_core::PromotionState;
use provenance_core::{
    ArtifactChangeType, ContributionStance, EvidenceQuality, IdeationEvidenceType,
    IdeationTargetType, ProposalType, SpeculationMarker, UncertaintyLevel,
};
use serde::Serialize;

pub(in crate::porcelain::schema) const IDEATION_TARGET_TYPES: [IdeationTargetType; 7] = [
    IdeationTargetType::Source,
    IdeationTargetType::Requirement,
    IdeationTargetType::Resolution,
    IdeationTargetType::Rule,
    IdeationTargetType::Topic,
    IdeationTargetType::Question,
    IdeationTargetType::Domain,
];

// Schema generation needs this concrete replica of the evidence domain that
// `rule_positive_evidence` decides in provenance-core. The conformance test in
// `schema/tests.rs` holds it to the core enum in both directions.
pub(in crate::porcelain::schema) const IDEATION_EVIDENCE_TYPES: [IdeationEvidenceType; 6] = [
    IdeationEvidenceType::Source,
    IdeationEvidenceType::Artifact,
    IdeationEvidenceType::ThreadMessage,
    IdeationEvidenceType::DomainKnowledge,
    IdeationEvidenceType::Unsupported,
    IdeationEvidenceType::Exploratory,
];

pub(in crate::porcelain::schema) const ARTIFACT_CHANGE_TYPES: [ArtifactChangeType; 4] = [
    ArtifactChangeType::Create,
    ArtifactChangeType::Update,
    ArtifactChangeType::Remove,
    ArtifactChangeType::None,
];

pub(in crate::porcelain::schema) const CONTRIBUTION_STANCES: [ContributionStance; 4] = [
    ContributionStance::Support,
    ContributionStance::Oppose,
    ContributionStance::Mixed,
    ContributionStance::NeedsMoreEvidence,
];

// This replica must name exactly the evidence types that
// `rule_positive_evidence` counts as speculation. The schema conformance test
// holds it to that domain in both directions.
pub(in crate::porcelain::schema) const SPECULATION_MARKERS: [SpeculationMarker; 2] = [
    SpeculationMarker::Unsupported,
    SpeculationMarker::Exploratory,
];

pub(in crate::porcelain::schema) const UNCERTAINTY_LEVELS: [UncertaintyLevel; 3] = [
    UncertaintyLevel::Low,
    UncertaintyLevel::Medium,
    UncertaintyLevel::High,
];

pub(in crate::porcelain::schema) const EVIDENCE_QUALITIES: [EvidenceQuality; 4] = [
    EvidenceQuality::Strong,
    EvidenceQuality::Mixed,
    EvidenceQuality::Weak,
    EvidenceQuality::Unsupported,
];

pub(in crate::porcelain::schema) const PROPOSAL_TYPES: [ProposalType; 6] = [
    ProposalType::RequirementCandidate,
    ProposalType::ResolutionCandidate,
    ProposalType::RuleCandidate,
    ProposalType::SourceGap,
    ProposalType::Question,
    ProposalType::NoAction,
];

#[cfg(test)]
pub(in crate::porcelain::schema) const PROMOTION_STATES: [PromotionState; 7] = [
    PromotionState::Proposed,
    PromotionState::Asserted,
    PromotionState::Accepted,
    PromotionState::Rejected,
    PromotionState::Deferred,
    PromotionState::Duplicate,
    PromotionState::Superseded,
];

pub(in crate::porcelain::schema) fn enum_names<T: Serialize>(variants: &[T]) -> Vec<String> {
    variants
        .iter()
        .map(|variant| {
            serde_json::to_value(variant)
                .expect("schema enum variant should serialize")
                .as_str()
                .expect("schema enum variant should serialize as a string")
                .to_string()
        })
        .collect()
}
