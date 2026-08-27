//! The projection digest: one hash over every family `SQLite` stores.
//!
//! This is a second digest domain beside `graph_digest`, built from the
//! same canonical serialization. Input is the `PROJECTION_FAMILIES` table:
//! all stored families, all manifest scopes, serialized family by family,
//! records sorted by scope then canonical id. Identical canonical bytes
//! therefore produce an identical digest, and any record change inside a
//! stored family moves it.

use crate::canonical_digest::{canonical_bytes, digest};
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;

use super::projection_families::{family_records, PROJECTION_FAMILIES};

/// Hashes the full projection state behind `layout`.
pub fn projection_digest(layout: &ProvenanceLayout) -> anyhow::Result<String> {
    let store = StateStore::new(layout.clone());
    let manifest = store.manifest()?;
    let mut families = Vec::new();
    for family in PROJECTION_FAMILIES {
        let mut records = family_records(family, &store, &manifest.scopes)?;
        records.sort_by_key(record_key);
        families.push(serde_json::json!({
            "family": family.name,
            "records": records,
        }));
    }
    Ok(digest(&canonical_bytes(&serde_json::json!({
        "families": families,
    }))?))
}

pub fn record_key(value: &serde_json::Value) -> (String, String) {
    (
        value
            .get("scope_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    )
}
