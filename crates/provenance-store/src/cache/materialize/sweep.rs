//! The byte-verify sweep over projection families.
//!
//! Every full-freshness pass reads the complete canonical bytes of every
//! stored (scope, family) and hashes them. Size and mtime are recorded as
//! diagnostic metadata and never license a skip: a write journal cannot
//! prove absence of writes that bypass it, so content hashes are the only
//! comparison the sweep trusts.

use crate::cache::projection_families::ProjectionFamily;
use crate::canonical_digest::sha256;
use crate::layout::ProvenanceLayout;
use provenance_core::ScopeId;
use std::time::SystemTime;

/// Diagnostic baseline for one (scope, family) shard.
#[derive(Debug, Clone)]
pub(super) struct FamilyBaseline {
    pub family: &'static str,
    pub digest: String,
    pub record_count: i64,
    pub size_bytes: i64,
    pub mtime_ns: i64,
}
fn placeholder_scope() -> &'static ScopeId {
    static PLACEHOLDER: std::sync::OnceLock<ScopeId> = std::sync::OnceLock::new();
    PLACEHOLDER.get_or_init(|| ScopeId::new("global").expect("literal scope id parses"))
}

/// Hashes a family's shard bytes and records the diagnostic metadata.
///
/// A global family covers every scope in one shard, so its baseline is
/// read once and stored under the empty scope key.
pub(super) fn shard_baseline(
    family: &ProjectionFamily,
    layout: &ProvenanceLayout,
    scope: Option<&ScopeId>,
) -> FamilyBaseline {
    let path = scope.map_or_else(
        || (family.shard)(layout, placeholder_scope()),
        |scope| (family.shard)(layout, scope),
    );
    let bytes = std::fs::read(&path).unwrap_or_default();
    let metadata = std::fs::metadata(&path).ok();
    FamilyBaseline {
        family: family.name,
        digest: format!("sha256:{}", sha256(&bytes)),
        record_count: i64::try_from(
            bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .count(),
        )
        .unwrap_or(i64::MAX),
        size_bytes: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
        mtime_ns: metadata
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |duration| {
                i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
            }),
    }
}

/// Mints one projection-instance identifier.
///
/// The instance scopes serial comparison: after total cache loss a fresh
/// database mints a fresh identifier, so a client holding stamps from the
/// two databases can see they are not comparable. Entropy comes from the
/// process id, the wall clock, and the address of a local, so two
/// databases minted in the same nanosecond still differ.
pub(super) fn mint_instance_id() -> anyhow::Result<String> {
    let local = 0u8;
    let entropy = format!(
        "{}-{}-{:p}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        &local
    );
    Ok(format!("pinst_{}", sha256(entropy.as_bytes())))
}
