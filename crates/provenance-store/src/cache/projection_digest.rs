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
/// The serialized form of this struct is the revision digest's input, so
/// its field set is part of the digest contract.
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
        for scope in &sorted {
            let (bytes, record_count) = family.canonical_records(store, scope)?;
            digests.push(FamilyContentDigest {
                kind: family,
                family: family.family_name(),
                scope_id: scope.as_str().to_string(),
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

/// Rebuilds the revision digest from stored family rows without parsing a
/// shard.
///
/// Rows may arrive in any order. An empty content digest means the row
/// predates the column. The caller must re-derive that family first.
pub fn revision_digest_from_stored_rows(
    rows: &[(String, String, String, i64)],
) -> anyhow::Result<String> {
    let mut by_key = std::collections::BTreeMap::new();
    for (scope_id, family, content_digest, record_count) in rows {
        anyhow::ensure!(
            !content_digest.is_empty(),
            "family `{family}` scope `{scope_id}` has no stored content digest"
        );
        by_key.insert(
            (family.as_str(), scope_id.as_str()),
            (content_digest, record_count),
        );
    }
    let mut scopes: Vec<&str> = rows
        .iter()
        .filter(|(scope_id, ..)| !scope_id.is_empty())
        .map(|(scope_id, ..)| scope_id.as_str())
        .collect();
    scopes.sort_unstable();
    scopes.dedup();
    let mut families = Vec::new();
    for family in ProjectionFamily::ALL {
        for scope_id in scopes.clone() {
            let (digest, record_count) =
                by_key
                    .get(&(family.family_name(), scope_id))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "no stored digest row for family `{}` scope `{scope_id}`",
                            family.family_name()
                        )
                    })?;
            families.push(FamilyContentDigest {
                kind: family,
                family: family.family_name(),
                scope_id: scope_id.to_string(),
                digest: (*digest).clone(),
                record_count: u64::try_from(**record_count)?,
            });
        }
    }
    revision_digest(&families)
}
