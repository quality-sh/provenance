//! The closed word lists of the four graph records.

use serde::{Deserialize, Serialize};

use super::super::parsing::parse_enum_word;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    #[serde(rename = "policy")]
    Policy,
    #[serde(rename = "document")]
    Document,
    #[serde(rename = "legislation")]
    Legislation,
    #[serde(rename = "company_agreement")]
    CompanyAgreement,
    #[serde(rename = "system_state")]
    SystemState,
    #[serde(rename = "external_integration")]
    ExternalIntegration,
    #[serde(rename = "domain_knowledge")]
    DomainKnowledge,
    #[serde(rename = "project_artifact")]
    ProjectArtifact,
    #[serde(rename = "incident")]
    Incident,
    #[serde(rename = "api_spec")]
    ApiSpec,
}

impl SourceType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        parse_enum_word(value)
    }

    /// The wire value that the typed-spec protocol carries for this type.
    /// `parse` accepts each value that this function gives.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Document => "document",
            Self::Legislation => "legislation",
            Self::CompanyAgreement => "company_agreement",
            Self::SystemState => "system_state",
            Self::ExternalIntegration => "external_integration",
            Self::DomainKnowledge => "domain_knowledge",
            Self::ProjectArtifact => "project_artifact",
            Self::Incident => "incident",
            Self::ApiSpec => "api_spec",
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequirementStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "discovery")]
    Discovery,
    #[serde(rename = "refinement")]
    Refinement,
    #[serde(rename = "resolved")]
    Resolved,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "approved")]
    Approved,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "revised")]
    Revised,
    #[serde(rename = "superseded")]
    Superseded,
    #[serde(rename = "abandoned")]
    Abandoned,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionInputType {
    #[serde(rename = "regulatory")]
    Regulatory,
    #[serde(rename = "legal_advice")]
    LegalAdvice,
    #[serde(rename = "commercial")]
    Commercial,
    #[serde(rename = "benchmark")]
    Benchmark,
    #[serde(rename = "technical")]
    Technical,
    #[serde(rename = "incident")]
    Incident,
    #[serde(rename = "source_material")]
    SourceMaterial,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "deprecated")]
    Deprecated,
    #[serde(rename = "archived")]
    Archived,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSeverity {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}
