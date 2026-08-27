use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use super::ids::{SchemaVersion, ScopeId};
use super::rbac::{Assignment, RbacSection};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoPathPrefix(Utf8PathBuf);

impl RepoPathPrefix {
    pub fn new(value: impl Into<Utf8PathBuf>) -> Self {
        Self(value.into())
    }
    pub fn as_path(&self) -> &camino::Utf8Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub id: ScopeId,
    pub path_prefix: RepoPathPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: SchemaVersion,
    pub scopes: Vec<Scope>,
    #[serde(default)]
    pub disposition_actor_ids: Vec<String>,
    /// Git-review-only RBAC grants. `skip_serializing_if` keeps init output
    /// byte-stable for repositories without the section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rbac: Option<RbacSection>,
}

impl Manifest {
    pub fn default_with_scope(scope: ScopeId, path_prefix: RepoPathPrefix) -> Self {
        Self {
            schema_version: SchemaVersion(1),
            scopes: vec![Scope {
                id: scope,
                path_prefix,
            }],
            disposition_actor_ids: Vec::new(),
            rbac: None,
        }
    }

    /// The grants this manifest carries, resolved for the ratification rule:
    /// legacy allowlist when no section is present, assignments otherwise.
    #[must_use]
    pub fn disposition_ratification(&self) -> DispositionRatification<'_> {
        self.rbac.as_ref().map_or_else(
            || DispositionRatification::LegacyAllowlist(&self.disposition_actor_ids),
            |section| DispositionRatification::RbacAssignments(&section.assignments),
        )
    }
}

/// The resolved authority a repository recognizes for recording dispositions.
///
/// Exactly one arm is live for any manifest, decided by the presence of the
/// `rbac` section. At the next protocol bump the legacy field and this
/// variant are removed together, and every feeder migrates in the same
/// change — the compiler keeps the two events from separating.
#[derive(Debug, Clone, Copy)]
pub enum DispositionRatification<'a> {
    /// The shipped-v1 manifest allowlist. Byte-for-byte law inside the
    /// one-window compatibility period; removed at the next protocol bump.
    LegacyAllowlist(&'a [String]),
    /// RBAC assignments: the recorded disposition actor must resolve to an
    /// assignment whose `identity_type` is `human`.
    RbacAssignments(&'a [Assignment]),
}
