//! The mutation backstop: the one gate every store write passes.
//!
//! On a repository without an `rbac` section nothing changes. On an
//! rbac-managed repository the gate consults the one policy choke with the
//! caller's claim, the census family's capability, and the scope the write
//! touches, and refuses a missing or unauthorized claim before any byte
//! moves. The check runs inside the publication lock, against manifest bytes
//! no concurrent writer can move.

use provenance_core::{authorize, Capability, RbacClaim, RbacResource, ScopeId};

use super::StateStore;

/// One mutation request at the store primitives: the caller's claim, the
/// census family's capability, and the scope the write lands in.
///
/// The capability is named by the mutating entry point from the operation
/// census; a verb outside the census has no family, and adding one is a
/// reviewed change, which is what makes default-deny mechanical.
#[derive(Clone)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct MutationAuth<'a> {
    claim: Option<&'a RbacClaim>,
    capability: Capability,
    scope: ScopeId,
}

impl<'a> MutationAuth<'a> {
    pub(crate) fn new(
        claim: Option<&'a RbacClaim>,
        capability: Capability,
        scope: &ScopeId,
    ) -> Self {
        Self {
            claim,
            capability,
            scope: scope.clone(),
        }
    }
}

impl StateStore {
    /// Runs the policy choke for a scoped mutation. No-op without the
    /// section; refuses otherwise when the claim is missing or unauthorized.
    #[allow(clippy::needless_pass_by_value)] // the value is small; the signature reads as a request token
    pub(crate) fn ensure_mutation_authorized(&self, auth: MutationAuth<'_>) -> anyhow::Result<()> {
        let manifest = self.manifest()?;
        let Some(section) = &manifest.rbac else {
            return Ok(());
        };
        authorize(
            auth.claim,
            section,
            auth.capability,
            RbacResource::Scope(&auth.scope),
        )
    }

    /// The gate for repo-global writes — the manifest itself, the project
    /// dictionary, an import swap. Settled Option A rule: the capability must
    /// be held on every scope then listed, so adding a scope narrows
    /// repo-global authority until grants cover it.
    ///
    /// Public to the workspace because the outlier writers that need it (the
    /// import swap, the dictionary reference) live in the CLI layer.
    pub fn ensure_repo_global_mutation(
        &self,
        claim: Option<&RbacClaim>,
        capability: Capability,
    ) -> anyhow::Result<()> {
        let manifest = self.manifest()?;
        let Some(section) = &manifest.rbac else {
            return Ok(());
        };
        let scopes: Vec<ScopeId> = manifest
            .scopes
            .iter()
            .map(|scope| scope.id.clone())
            .collect();
        authorize(
            claim,
            section,
            capability,
            RbacResource::RepoGlobal(&scopes),
        )
    }
}
