//! Opaque repository selection and scoped read settings.
use super::StampPolicy;
use serde::{Deserialize, Serialize};

/// Which freshness step a read runs before it answers.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessPolicy {
    /// Run catch-up under the publication guard, then answer.
    #[default]
    CatchUp,
    /// Answer at the stored serial without a freshness step.
    AnnotateOnly,
    /// Refuse when the stored projection differs from canonical state.
    RefuseStale,
}

impl FreshnessPolicy {
    /// The stamp word for a step that ran as asked.
    pub const fn word(self) -> StampPolicy {
        match self {
            Self::CatchUp => StampPolicy::CatchUp,
            Self::AnnotateOnly => StampPolicy::AnnotateOnly,
            Self::RefuseStale => StampPolicy::RefuseStale,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RepositoryContext {
    pub repository: String,
    pub scope: String,
    #[serde(default)]
    pub freshness: Option<FreshnessPolicy>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InfoRequest {}

/// External metadata names the requested target, never a host path.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RepositoryInfo {
    pub engine_version: String,
    pub protocol_version: u32,
    pub state_schema_version: u32,
    pub repository: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RepositoryTarget {
    pub repository: String,
}

/// Scope selection for operations that do not use projection freshness.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RepositoryScope {
    pub repository: String,
    pub scope: String,
}

/// Existing verification lists remain complete and have no page controls.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VerificationListRequest {
    #[serde(default)]
    pub rule: Option<crate::StableId>,
}
