//! The one authorization policy for manifest RBAC grants.
//!
//! Everything RBAC refuses is decided here: the manifest read law that keeps
//! legacy and RBAC regimes unambiguous, the well-formedness law for the
//! section itself, the single `authorize` choke every mutating primitive
//! consults, and the human-ratification identity check. No refusal logic
//! lives outside this module.

use std::collections::BTreeSet;

use super::types::{Assignment, Capability, RbacClaim, RbacResource, RbacSection};
use crate::model::ids::ScopeId;

/// Fixed refusal wording for a manifest holding both regimes at once.
pub const AMBIGUOUS_MANIFEST_REFUSAL: &str = "ambiguous manifest: disposition_actor_ids and rbac.assignments are both present; move disposition actors into rbac assignments and remove disposition_actor_ids";

/// Fixed refusal wording for a mutation that arrives with no claim at all.
pub const MISSING_CLAIM_REFUSAL: &str =
    "rbac: no actor claim supplied for a mutating operation on an rbac-managed repository";

/// Tail shared by every ratification refusal; the actor id is named before it.
pub const RATIFICATION_REFUSAL_TAIL: &str =
    "needs an assignment with identity_type human to end a live proposal";

/// The manifest read law: refuse a manifest that holds a non-empty legacy
/// disposition allowlist and an `rbac` section at the same time.
///
/// An empty legacy array beside `rbac` is unambiguous — fresh inits ship
/// `"disposition_actor_ids": []` — and only a non-empty list could carry
/// authority the section does not know about. Every manifest reader calls
/// this; the list is part of the acceptance checklist.
pub fn ensure_unambiguous_rbac(
    disposition_actor_ids: &[String],
    rbac: Option<&RbacSection>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        rbac.is_none() || disposition_actor_ids.is_empty(),
        "{AMBIGUOUS_MANIFEST_REFUSAL}"
    );
    Ok(())
}

/// The section well-formedness law: actor ids are non-empty, and no
/// `(actor_id, scope)` pair may be granted twice inside one section.
pub fn ensure_rbac_section_well_formed(section: &RbacSection) -> anyhow::Result<()> {
    let mut seen: std::collections::BTreeSet<(String, String)> = BTreeSet::new();
    for assignment in &section.assignments {
        anyhow::ensure!(
            !assignment.actor_id.trim().is_empty(),
            "rbac assignment actor_id must not be empty"
        );
        for scope in &assignment.scopes {
            ScopeId::new(scope)?;
            anyhow::ensure!(
                seen.insert((assignment.actor_id.clone(), scope.clone())),
                "duplicate rbac grant: actor {} already names scope {}",
                assignment.actor_id,
                scope
            );
        }
    }
    Ok(())
}

/// The one policy choke: does this claimed principal hold the needed
/// capability on this resource?
///
/// A missing claim refuses with its own golden, distinct from a wrong
/// principal. Repo-global resources demand the capability on every scope
/// then listed (Option A); the refusal names the first uncovered scope.
/// `read` gates nothing observable in v1 — reads ship ungated — so this
/// function is only consulted for mutations.
pub fn authorize(
    claim: Option<&RbacClaim>,
    section: &RbacSection,
    needed: Capability,
    resource: RbacResource<'_>,
) -> anyhow::Result<()> {
    let claim = claim.ok_or_else(|| anyhow::anyhow!("{MISSING_CLAIM_REFUSAL}"))?;
    let assignments = section.assignments_for(&claim.actor_id);
    match resource {
        RbacResource::Scope(scope) => {
            holds_on_scope(&assignments, &claim.actor_id, needed, scope.as_str())
        }
        RbacResource::RepoGlobal(scopes) => {
            for scope in scopes {
                holds_on_scope(&assignments, &claim.actor_id, needed, scope.as_str())?;
            }
            Ok(())
        }
    }
}

fn holds_on_scope(
    assignments: &[&Assignment],
    actor_id: &str,
    needed: Capability,
    scope: &str,
) -> anyhow::Result<()> {
    let held = assignments.iter().any(|assignment| {
        assignment.capabilities.contains(&needed) && assignment.scopes.iter().any(|s| s == scope)
    });
    anyhow::ensure!(
        held,
        "rbac: actor {actor_id} does not hold capability {} on scope {scope}",
        needed.as_str()
    );
    Ok(())
}

/// The human-ratification identity check: a disposition's recorded actor must
/// resolve to an assignment whose `identity_type` is `human`.
///
/// An assignment with no `identity_type` refuses with the same wording, and
/// so does an actor with no assignment at all — the check fails closed
/// (decision D2). This is the rule that replaces the legacy allowlist when
/// the next protocol bump removes `disposition_actor_ids`; it already governs
/// every rbac-managed repository.
pub fn ensure_disposition_actor_is_human(
    actor_id: &str,
    assignments: &[Assignment],
) -> anyhow::Result<()> {
    let is_human = assignments.iter().any(|assignment| {
        assignment.actor_id == actor_id
            && assignment.identity_type == Some(crate::model::ideation::IdentityType::Human)
    });
    anyhow::ensure!(
        is_human,
        "rbac: disposition actor {actor_id} {RATIFICATION_REFUSAL_TAIL}"
    );
    Ok(())
}
