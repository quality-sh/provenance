//! The closed word lists of the four graph records.

use serde::{Deserialize, Serialize};

use super::super::parsing::normalize_enum_value;

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
        match normalize_enum_value(value).as_str() {
            "policy" => Ok(Self::Policy),
            "document" => Ok(Self::Document),
            "legislation" => Ok(Self::Legislation),
            "company_agreement" => Ok(Self::CompanyAgreement),
            "system_state" => Ok(Self::SystemState),
            "external_integration" => Ok(Self::ExternalIntegration),
            "domain_knowledge" => Ok(Self::DomainKnowledge),
            "project_artifact" => Ok(Self::ProjectArtifact),
            "incident" => Ok(Self::Incident),
            "api_spec" => Ok(Self::ApiSpec),
            _ => anyhow::bail!("source type must be a supported provenance source type"),
        }
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

impl RequirementStatus {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "active" => Ok(Self::Active),
            "discovery" => Ok(Self::Discovery),
            "refinement" => Ok(Self::Refinement),
            "resolved" => Ok(Self::Resolved),
            _ => anyhow::bail!(
                "requirement status must be active, discovery, refinement, or resolved"
            ),
        }
    }
}

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

impl ResolutionStatus {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "draft" => Ok(Self::Draft),
            "review" => Ok(Self::Review),
            "proposed" => Ok(Self::Proposed),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "revised" => Ok(Self::Revised),
            "superseded" => Ok(Self::Superseded),
            "abandoned" => Ok(Self::Abandoned),
            _ => anyhow::bail!("resolution status must be draft, review, proposed, approved, rejected, revised, superseded, or abandoned"),
        }
    }
}

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

impl ResolutionInputType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "regulatory" => Ok(Self::Regulatory),
            "legal_advice" => Ok(Self::LegalAdvice),
            "commercial" => Ok(Self::Commercial),
            "benchmark" => Ok(Self::Benchmark),
            "technical" => Ok(Self::Technical),
            "incident" => Ok(Self::Incident),
            "source_material" => Ok(Self::SourceMaterial),
            _ => anyhow::bail!("resolution input type must be regulatory, legal_advice, commercial, benchmark, technical, incident, or source_material"),
        }
    }
}

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

impl RuleStatus {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "draft" => Ok(Self::Draft),
            "review" => Ok(Self::Review),
            "active" => Ok(Self::Active),
            "deprecated" => Ok(Self::Deprecated),
            "archived" => Ok(Self::Archived),
            _ => {
                anyhow::bail!("rule status must be draft, review, active, deprecated, or archived")
            }
        }
    }
}

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

impl RuleSeverity {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => anyhow::bail!("severity must be low, medium, high, or critical"),
        }
    }
}
