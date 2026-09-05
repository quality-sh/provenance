use camino::Utf8PathBuf;
use provenance_macros::ProjectionRow;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

use super::{SchemaVersion, ScopeId, StableId};

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}

/// How a verification binding supports its Rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    Exhaustion,
    Property,
    Examples,
    Conformance,
    Construction,
    Proof,
}

impl fmt::Display for VerificationMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exhaustion => "exhaustion",
            Self::Property => "property",
            Self::Examples => "examples",
            Self::Conformance => "conformance",
            Self::Construction => "construction",
            Self::Proof => "proof",
        })
    }
}

impl FromStr for VerificationMethod {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "exhaustion" => Ok(Self::Exhaustion),
            "property" => Ok(Self::Property),
            "examples" => Ok(Self::Examples),
            "conformance" => Ok(Self::Conformance),
            "construction" => Ok(Self::Construction),
            "proof" => Ok(Self::Proof),
            other => anyhow::bail!("invalid verification method: {other}"),
        }
    }
}

/// One owner-local path to a language-authored declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct DeclarationAddress(Vec<String>);

impl DeclarationAddress {
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> anyhow::Result<Self> {
        let segments = segments.into_iter().map(Into::into).collect::<Vec<_>>();
        anyhow::ensure!(
            !segments.is_empty() && segments.iter().all(|segment| !segment.trim().is_empty()),
            "declaration address segments must not be empty"
        );
        Ok(Self(segments))
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for DeclarationAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(Vec::<String>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// The lifecycle state of one callback-backed verification run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationRunStatus {
    Running,
    Passed,
    Failed,
}

impl VerificationRunStatus {
    pub fn parse_completion(value: &str) -> anyhow::Result<Self> {
        match value {
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            _ => anyhow::bail!("verification completion status must be passed or failed"),
        }
    }
}

/// Volatile evidence from one language-owned verification callback.
///
/// Runs live in Provenance's derived cache rather than canonical state: a
/// local test run must not dirty the repository. `rule_id` is the join back
/// to the canonical graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRun {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_id: Option<StableId>,
    pub rule_id: StableId,
    pub method: String,
    pub declared_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<Utf8PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub status: VerificationRunStatus,
    pub started_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One durable language-authored relationship from a code site to a Rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ProjectionRow)]
#[table("verification_bindings")]
pub struct VerificationBinding {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub rule_id: StableId,
    pub key: String,
    pub method: VerificationMethod,
    pub declared_by: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub retired: bool,
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// One record of a Rule whose evidence needs review because the Requirement
/// it serves was restated.
///
/// The record keeps why review was asked for. A verification run arriving
/// after the change clears it in place rather than removing the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ProjectionRow)]
#[table("requirement_reviews")]
pub struct RequirementReview {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub rule_id: StableId,
    pub requirement_id: StableId,
    pub field: String,
    pub before: String,
    pub after: String,
    pub changed_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared_by_run: Option<StableId>,
}

/// One canonical primary implementation relationship from an exported
/// production symbol to a Rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ProjectionRow)]
#[table("implementation_bindings")]
pub struct ImplementationBinding {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub rule_id: StableId,
    pub declared_by: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub retired: bool,
    pub file: Utf8PathBuf,
    pub symbol: String,
}
