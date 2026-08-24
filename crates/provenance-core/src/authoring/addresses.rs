//! The four legal declaration-address shapes, single-homed in the kernel.
//!
//! The store calls into these constructors at ingestion; no other copy of
//! the shapes exists.

use crate::model::DeclarationAddress;
use crate::protocol::TypedRuleInput;

/// `[spec, "source", key]`.
pub fn source_address(spec: &str, key: &str) -> anyhow::Result<DeclarationAddress> {
    DeclarationAddress::new([spec, "source", key])
}

/// `[spec, "requirement", key]`.
pub fn requirement_address(spec: &str, key: &str) -> anyhow::Result<DeclarationAddress> {
    DeclarationAddress::new([spec, "requirement", key])
}

/// `[spec, "rule", key]` — the shape for a Rule with several owners.
pub fn shared_rule_address(spec: &str, key: &str) -> anyhow::Result<DeclarationAddress> {
    DeclarationAddress::new([spec, "rule", key])
}

/// `[spec, "requirement", requirement, "rule", key]` — one owner.
pub fn local_rule_address(
    spec: &str,
    requirement: &str,
    key: &str,
) -> anyhow::Result<DeclarationAddress> {
    DeclarationAddress::new([spec, "requirement", requirement, "rule", key])
}

/// Infers or validates the address of one rule declaration.
///
/// An explicit address must be the shared shape, or the local shape under
/// the rule's single requirement. Without one, a single-requirement rule is
/// local and a multi-requirement rule is shared.
pub fn rule_address(
    spec: &str,
    declaration: &TypedRuleInput,
) -> anyhow::Result<DeclarationAddress> {
    let inferred = inferred_address(spec, &declaration.requirements, &declaration.key)?;
    let Some(explicit) = &declaration.address else {
        return Ok(inferred);
    };
    let shared = shared_rule_address(spec, &declaration.key)?;
    if explicit == &shared {
        return Ok(explicit.clone());
    }
    if let [requirement] = declaration.requirements.as_slice() {
        let local = local_rule_address(spec, requirement, &declaration.key)?;
        anyhow::ensure!(
            explicit == &local,
            "rule `{}` address must be `{}` or `{}` for requirement `{requirement}`",
            declaration.key,
            shared.segments().join("/"),
            local.segments().join("/")
        );
        return Ok(explicit.clone());
    }
    anyhow::bail!(
        "rule `{}` with several requirements must use shared address `{}`",
        declaration.key,
        shared.segments().join("/")
    )
}

/// Names the owning requirement of a local rule address.
pub fn local_parent(address: &DeclarationAddress) -> Option<String> {
    match address.segments() {
        [_, kind, requirement, rule, _] if kind == "requirement" && rule == "rule" => {
            Some(requirement.clone())
        }
        _ => None,
    }
}

fn inferred_address(
    spec: &str,
    requirements: &[String],
    key: &str,
) -> anyhow::Result<DeclarationAddress> {
    match requirements {
        [requirement] => local_rule_address(spec, requirement, key),
        [] => anyhow::bail!("rule `{key}` must refine at least one requirement"),
        _ => shared_rule_address(spec, key),
    }
}
