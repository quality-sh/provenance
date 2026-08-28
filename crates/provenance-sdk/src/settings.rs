//! Environment settings, in the TypeScript SDK's vocabulary.
//!
//! `PROVENANCE_BIN` names an external engine binary for out-of-process
//! SDKs; the Rust SDK runs in process and does not read it.

use camino::Utf8PathBuf;
use provenance_core::{RbacClaim, ScopeId};

/// What the process environment says about repository, scope, and owners.
#[derive(Debug, Clone)]
pub struct Settings {
    /// `PROVENANCE_REPO`; `None` lets the operations discover the
    /// nearest enclosing project.
    pub repository: Option<Utf8PathBuf>,
    /// `PROVENANCE_SCOPE`, default `default`.
    pub scope: String,
    /// `PROVENANCE_SPEC_OWNER`, default `spec://rust`.
    pub owner: String,
    /// `PROVENANCE_VERIFICATION_OWNER`, default `ci://rust`.
    pub verification_owner: String,
    /// `PROVENANCE_ACTOR_ID` — the mutating principal's claim, sent with
    /// every mutating request the facade makes. Absent when unset, and an
    /// unset claim refuses on an rbac-managed repository.
    pub actor: Option<RbacClaim>,
}

impl Settings {
    pub fn from_env() -> Self {
        Self {
            repository: std::env::var("PROVENANCE_REPO").ok().map(Utf8PathBuf::from),
            scope: std::env::var("PROVENANCE_SCOPE").unwrap_or_else(|_| "default".to_string()),
            owner: std::env::var("PROVENANCE_SPEC_OWNER")
                .unwrap_or_else(|_| "spec://rust".to_string()),
            verification_owner: std::env::var("PROVENANCE_VERIFICATION_OWNER")
                .unwrap_or_else(|_| "ci://rust".to_string()),
            actor: std::env::var("PROVENANCE_ACTOR_ID")
                .ok()
                .filter(|id| !id.trim().is_empty())
                .and_then(|id| RbacClaim::new(id).ok()),
        }
    }

    pub fn scope_id(&self) -> anyhow::Result<ScopeId> {
        ScopeId::new(self.scope.clone())
    }
}
