//! The revision digest: one deterministic hash over everything stored.
//!
//! The digest domain is the `PROJECTION_FAMILIES` table and nothing else. It
//! walks every family over every scope, serializes each record list sorted
//! by canonical id through the one canonical writer, and hashes the per-family
//! digests into one revision digest. This is a different domain from
//! `graph_digest`: that one pins the exportable graph; this one attests the
//! whole projection, collaboration and ideation records included.

use super::ProjectionFamily;
use crate::{canonical_digest, state_store::StateStore};
use provenance_core::ScopeId;
use serde::Serialize;

/// The content digest of one (family, scope) record list.
///
/// `scope_id` is empty for the global edges family. The serialized form of
/// this struct is the revision digest's input, so its field set is part of
/// the digest contract.
#[derive(Debug, Clone, Serialize)]
pub struct FamilyContentDigest {
    #[serde(skip)]
    pub kind: ProjectionFamily,
    pub family: &'static str,
    pub scope_id: String,
    pub digest: String,
    pub record_count: u64,
}

/// One content digest per (family, scope), in table order with scopes
/// sorted, so two repositories holding the same records produce the same
/// list.
pub fn family_content_digests(
    store: &StateStore,
    scopes: &[ScopeId],
) -> anyhow::Result<Vec<FamilyContentDigest>> {
    let mut sorted: Vec<&ScopeId> = scopes.iter().collect();
    sorted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut digests = Vec::new();
    for family in ProjectionFamily::ALL {
        let scope_keys: Vec<Option<&ScopeId>> = if family.is_scoped() {
            sorted.iter().map(|scope| Some(*scope)).collect()
        } else {
            vec![None]
        };
        for scope in scope_keys {
            let (bytes, record_count) = family.canonical_records(store, scope)?;
            digests.push(FamilyContentDigest {
                kind: family,
                family: family.family_name(),
                scope_id: scope.map(ScopeId::as_str).unwrap_or_default().to_string(),
                digest: canonical_digest::digest(&bytes),
                record_count,
            });
        }
    }
    Ok(digests)
}

/// The revision digest: canonical bytes of the family digest list, hashed.
pub fn revision_digest(families: &[FamilyContentDigest]) -> anyhow::Result<String> {
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&families)?,
    ))
}
