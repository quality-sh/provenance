//! Persistent identity resolution.
//!
//! The kernel admits declarations structurally at the existing pipeline
//! points; only this module maps admitted addresses to canonical ids,
//! migrates local and shared identities, and enforces ownership. Only
//! provenance-store determines persistent declaration identity.

use std::collections::{BTreeMap, BTreeSet};

use provenance_core::authoring::{self, addresses};
use provenance_core::{DeclarationAddress, StableId};
use provenance_macros::rule;
use sha2::{Digest, Sha256};

use super::super::{TypedRequirementInput, TypedRuleInput, TypedSourceInput};
use super::rule_addresses::migration_candidates;

pub(super) fn source_address(spec: &str, key: &str) -> anyhow::Result<DeclarationAddress> {
    addresses::source_address(spec, key)
}

pub(super) fn requirement_address(spec: &str, key: &str) -> anyhow::Result<DeclarationAddress> {
    addresses::requirement_address(spec, key)
}

pub(super) fn source_identity(input: &TypedSourceInput) -> (&str, Option<&str>) {
    (&input.key, input.id.as_deref())
}

pub(super) fn requirement_identity(input: &TypedRequirementInput) -> (&str, Option<&str>) {
    (&input.key, input.id.as_deref())
}

pub(super) use authoring::{normalize_rule_relationships, validate_references};

pub(super) fn rule_declaration_ids(
    owner: &str,
    spec: &str,
    declarations: &[TypedRuleInput],
    existing: &BTreeMap<DeclarationAddress, StableId>,
) -> anyhow::Result<BTreeMap<DeclarationAddress, StableId>> {
    let mut ids = BTreeMap::new();
    let mut canonical = BTreeSet::new();
    let mut checker = authoring::RuleChecker::new();
    for declaration in declarations {
        let address = checker.admit(spec, declaration)?;
        let candidates = migration_candidates(spec, declaration, &address, existing)?;
        let canonical_key = match address.segments() {
            [_, _, requirement, _, _] => format!("{spec}/{requirement}/{}", declaration.key),
            _ => format!("{spec}/{}", declaration.key),
        };
        let id = resolve_rule_id(
            owner,
            &address,
            &canonical_key,
            declaration.id.as_deref(),
            existing,
            &candidates,
        )?;
        anyhow::ensure!(
            canonical.insert(id.as_str().to_string()),
            "two rule declarations resolve to id `{}`",
            id.as_str()
        );
        ids.insert(address, id);
    }
    Ok(ids)
}

fn resolve_rule_id(
    owner: &str,
    address: &DeclarationAddress,
    canonical_key: &str,
    explicit_id: Option<&str>,
    existing: &BTreeMap<DeclarationAddress, StableId>,
    candidates: &[StableId],
) -> anyhow::Result<StableId> {
    let explicit_id = explicit_id.map(StableId::new).transpose()?;
    if let Some(existing_id) = existing.get(address) {
        anyhow::ensure!(
            explicit_id.as_ref().is_none_or(|id| id == existing_id),
            "declaration `{}` already resolves to canonical id `{}`",
            address.segments().join("/"),
            existing_id.as_str()
        );
        return Ok(existing_id.clone());
    }
    if let Some(id) = explicit_id {
        anyhow::ensure!(
            candidates.is_empty() || candidates.contains(&id),
            "rule declaration `{}` can only select an existing migration candidate by explicit id",
            address.segments().join("/")
        );
        return Ok(id);
    }
    match candidates {
        [id] => return Ok(id.clone()),
        [_, _, ..] => anyhow::bail!(
            "rule declaration `{}` has ambiguous existing identities; provide an explicit existing id",
            address.segments().join("/")
        ),
        [] => {}
    }
    resolve_id("rule", owner, address, canonical_key, None, existing)
}

pub(super) fn declaration_ids<'a>(
    kind: &'static str,
    owner: &str,
    spec: &str,
    declarations: impl Iterator<Item = (&'a str, Option<&'a str>)>,
    existing: &BTreeMap<DeclarationAddress, StableId>,
) -> anyhow::Result<BTreeMap<String, StableId>> {
    let mut ids = BTreeMap::new();
    let mut canonical = BTreeSet::new();
    let mut checker = authoring::DeclarationChecker::new(kind);
    for (key, explicit_id) in declarations {
        let address = checker.admit(spec, key)?;
        let id = resolve_id(
            kind,
            owner,
            &address,
            &format!("{spec}/{key}"),
            explicit_id,
            existing,
        )?;
        anyhow::ensure!(
            canonical.insert(id.as_str().to_string()),
            "two {kind} declarations resolve to id `{}`",
            id.as_str()
        );
        ids.insert(key.to_string(), id);
    }
    Ok(ids)
}

/// Maps one admitted declaration address to its canonical id: the
/// existing id for a known address, the caller's explicit id, the
/// canonical key when it is well formed, or a digest-suffixed slug.
#[rule("rule_rust_store_owns_persistent_identity")]
fn resolve_id(
    kind: &str,
    owner: &str,
    address: &DeclarationAddress,
    canonical_key: &str,
    explicit_id: Option<&str>,
    existing: &BTreeMap<DeclarationAddress, StableId>,
) -> anyhow::Result<StableId> {
    let explicit_id = explicit_id.map(StableId::new).transpose()?;
    if let Some(existing_id) = existing.get(address) {
        anyhow::ensure!(
            explicit_id.as_ref().is_none_or(|id| id == existing_id),
            "declaration `{}` already resolves to canonical id `{}`",
            address.segments().join("/"),
            existing_id.as_str()
        );
        return Ok(existing_id.clone());
    }
    if let Some(id) = explicit_id {
        return Ok(id);
    }

    if let Ok(id) = StableId::new(canonical_key) {
        return Ok(id);
    }
    let slug = canonical_key
        .chars()
        .map(|character| match character {
            character if character.is_ascii_alphanumeric() => character.to_ascii_lowercase(),
            '-' | '_' => character,
            _ => '_',
        })
        .collect::<String>();
    let identity = format!("{owner}\0{}", address.segments().join("/"));
    let digest = format!("{:x}", Sha256::digest(identity.as_bytes()));
    StableId::new(format!("{kind}_{slug}_{}", &digest[..10]))
}

pub(super) fn owned_declaration_ids<'a, T: 'a>(
    owner: &str,
    records: &'a [T],
    fields: impl Fn(
        &'a T,
    ) -> (
        &'a StableId,
        Option<&'a str>,
        Option<&'a DeclarationAddress>,
    ),
) -> anyhow::Result<BTreeMap<DeclarationAddress, StableId>> {
    let mut ids = BTreeMap::new();
    for record in records {
        let (id, declared_by, address) = fields(record);
        if declared_by != Some(owner) {
            continue;
        }
        let Some(address) = address else {
            continue;
        };
        anyhow::ensure!(
            ids.insert(address.clone(), id.clone()).is_none(),
            "declaration `{}` resolves to more than one canonical id",
            address.segments().join("/")
        );
    }
    Ok(ids)
}

pub(super) fn validate_ownership<'a, T: 'a>(
    owner: &str,
    records: &'a [T],
    desired_ids: impl Iterator<Item = &'a StableId>,
    fields: impl Fn(&'a T) -> (&'a StableId, Option<&'a str>),
) -> anyhow::Result<()> {
    for desired_id in desired_ids {
        let Some(record) = records.iter().find(|record| fields(record).0 == desired_id) else {
            continue;
        };
        let (_, declared_by) = fields(record);
        anyhow::ensure!(
            declared_by == Some(owner),
            "record `{}` is not owned by `{owner}` (declared_by: {})",
            desired_id.as_str(),
            declared_by.unwrap_or("unowned")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{declaration_ids, requirement_address, rule_declaration_ids};
    use crate::state_store::typed_specs::rule_address;
    use crate::state_store::TypedRuleInput;
    use provenance_core::StableId;
    use provenance_macros::verifies;

    #[test]
    fn implicit_ids_are_scoped_to_the_spec_address() {
        let existing = BTreeMap::new();
        let first = declaration_ids(
            "requirement",
            "spec://typescript",
            "sharing",
            std::iter::once(("sharing", None)),
            &existing,
        )
        .unwrap();
        let second = declaration_ids(
            "requirement",
            "spec://typescript",
            "sessions",
            std::iter::once(("sharing", None)),
            &existing,
        )
        .unwrap();
        assert_ne!(first["sharing"], second["sharing"]);

        let declaration = TypedRuleInput {
            key: "expiry".to_string(),
            id: None,
            address: None,
            requirement: None,
            requirements: vec!["sharing".to_string()],
            statement: "Share links expire".to_string(),
            name: None,
            description: None,
            implementation: None,
        };
        let first = rule_declaration_ids(
            "spec://typescript",
            "sharing",
            std::slice::from_ref(&declaration),
            &existing,
        )
        .unwrap();
        let second = rule_declaration_ids(
            "spec://typescript",
            "sessions",
            std::slice::from_ref(&declaration),
            &existing,
        )
        .unwrap();
        let first_address = rule_address("sharing", &declaration).unwrap();
        let second_address = rule_address("sessions", &declaration).unwrap();
        assert_ne!(first[&first_address], second[&second_address]);
    }

    #[test]
    #[verifies("rule_rust_store_owns_persistent_identity", examples)]
    fn an_existing_address_keeps_its_canonical_id() {
        let address = requirement_address("share-links", "sharing").unwrap();
        let mut existing = BTreeMap::new();
        existing.insert(address, StableId::new("req_existing").unwrap());

        let ids = declaration_ids(
            "requirement",
            "spec://typescript",
            "share-links",
            std::iter::once(("sharing", None)),
            &existing,
        )
        .unwrap();

        assert_eq!(ids["sharing"].as_str(), "req_existing");
    }

    #[test]
    fn an_existing_address_cannot_be_remapped_to_another_explicit_id() {
        let address = requirement_address("share-links", "sharing").unwrap();
        let mut existing = BTreeMap::new();
        existing.insert(address, StableId::new("req_existing").unwrap());

        let error = declaration_ids(
            "requirement",
            "spec://typescript",
            "share-links",
            std::iter::once(("sharing", Some("req_other"))),
            &existing,
        )
        .unwrap_err();

        assert!(error.to_string().contains("already resolves"));
    }
}
