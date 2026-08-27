//! Closed wire types for the manifest `rbac` section and the claim that
//! travels with every mutating request.

use crate::model::ideation::IdentityType;
use crate::model::ids::ScopeId;
use serde::{Deserialize, Serialize};

/// The complete capability vocabulary. Closed by decision D1: no wildcard, no
/// delegation, no expiry, and no capability beyond these four words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    Read,
    Edit,
    Execute,
    ManifestWrite,
}

impl Capability {
    /// The wire and refusal spelling of this capability.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Execute => "execute",
            Self::ManifestWrite => "manifest-write",
        }
    }

    /// Parses the wire word, refusing anything outside the closed set.
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "read" => Ok(Self::Read),
            "edit" => Ok(Self::Edit),
            "execute" => Ok(Self::Execute),
            "manifest-write" => Ok(Self::ManifestWrite),
            _ => anyhow::bail!("capability must be read, edit, execute, or manifest-write"),
        }
    }
}

/// One principal's grants: positive capabilities on explicit manifest scopes.
///
/// `identity_type` reuses the core `IdentityType` enum. An assignment that
/// omits it fails closed for human-ratification operations (decision D2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub actor_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_type: Option<IdentityType>,
    pub capabilities: Vec<Capability>,
    pub scopes: Vec<String>,
}

/// The `rbac` section of the manifest: Git-review-only grant data. No engine
/// verb writes it (decision D6); it changes only through reviewed commits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RbacSection {
    pub assignments: Vec<Assignment>,
}

impl RbacSection {
    /// Every assignment naming this actor id.
    #[must_use]
    pub fn assignments_for<'a>(&'a self, actor_id: &str) -> Vec<&'a Assignment> {
        self.assignments
            .iter()
            .filter(|assignment| assignment.actor_id == actor_id)
            .collect()
    }
}

/// The mutating principal's attested identity, supplied top-down by the CLI
/// or SDK.
///
/// It is a claim about who acts, never proof: no authentication is performed
/// or implied. Principal ids pass through unchanged so external auth-provider
/// subjects stay compatible (decision D1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RbacClaim {
    pub actor_id: String,
}

impl RbacClaim {
    /// Builds a claim, refusing an empty actor id.
    pub fn new(actor_id: impl Into<String>) -> anyhow::Result<Self> {
        let actor_id = actor_id.into();
        anyhow::ensure!(!actor_id.trim().is_empty(), "actor id must not be empty");
        Ok(Self { actor_id })
    }
}

/// The resource a capability is asked for.
///
/// A scope resource is one manifest scope. A repo-global resource (the
/// manifest itself, `dictionary.json`, an import swap) spans every scope then
/// listed, and the settled Option A rule applies: the capability must be held
/// on every one of those scopes, so adding a scope narrows repo-global
/// authority until grants cover it.
#[derive(Debug, Clone, Copy)]
pub enum RbacResource<'a> {
    Scope(&'a ScopeId),
    RepoGlobal(&'a [ScopeId]),
}
