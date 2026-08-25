//! Rule-address access at the store seam.
//!
//! The kernel owns the address shapes and their validation; this module
//! keeps the store's call sites and the resolution-dependent migration
//! candidate lookup, which reads existing state and stays store-owned.

use std::collections::BTreeMap;

use provenance_core::authoring::addresses;
use provenance_core::{DeclarationAddress, StableId};

use super::super::TypedRuleInput;

pub(in crate::state_store) fn rule_address(
    spec: &str,
    declaration: &TypedRuleInput,
) -> anyhow::Result<DeclarationAddress> {
    addresses::rule_address(spec, declaration)
}

pub(super) fn local_parent(address: &DeclarationAddress) -> Option<String> {
    addresses::local_parent(address)
}

/// Finds the existing identities a relocated declaration could continue.
pub(super) fn migration_candidates(
    spec: &str,
    declaration: &TypedRuleInput,
    desired: &DeclarationAddress,
    existing: &BTreeMap<DeclarationAddress, StableId>,
) -> anyhow::Result<Vec<StableId>> {
    let shared = addresses::shared_rule_address(spec, &declaration.key)?;
    let candidate_addresses = if desired == &shared {
        declaration
            .requirements
            .iter()
            .map(|requirement| addresses::local_rule_address(spec, requirement, &declaration.key))
            .collect::<anyhow::Result<Vec<_>>>()?
    } else {
        vec![shared]
    };
    let candidates = candidate_addresses
        .iter()
        .filter_map(|address| existing.get(address))
        .map(|id| (id.as_str(), id))
        .collect::<BTreeMap<_, _>>();
    Ok(candidates.into_values().cloned().collect())
}
