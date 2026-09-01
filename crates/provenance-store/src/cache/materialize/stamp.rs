use crate::cache::{projection_digest, projection_digest::FamilyContentDigest};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

/// Writes the revision stamp the loaded rows answer under.
///
/// Runs inside the materialization transaction, so rows, the revision, and
/// the family baselines commit together or not at all. The instance id is
/// written once from OS entropy and never replaced while the database file
/// lives; it is what keeps serials from different database lifetimes apart.
pub(super) async fn write_stamp(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    layout: &ProvenanceLayout,
    scopes: &[ScopeId],
) -> anyhow::Result<()> {
    let families = projection_digest::family_content_digests(store, scopes)?;
    let revision = projection_digest::revision_digest(&families)?;

    sqlx::query("INSERT OR IGNORE INTO projection_instance (only_row, instance_id) VALUES (1, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&mut **tx)
        .await?;

    let last_serial: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(serial), 0) FROM projection_revision")
            .fetch_one(&mut **tx)
            .await?;
    sqlx::query("INSERT INTO projection_revision (serial, digest) VALUES (?, ?)")
        .bind(last_serial + 1)
        .bind(&revision)
        .execute(&mut **tx)
        .await?;

    sqlx::query("DELETE FROM projection_family_digests")
        .execute(&mut **tx)
        .await?;
    for family in &families {
        let (shard_digest, size_bytes, mtime_ns) = shard_baseline(layout, family)?;
        sqlx::query("INSERT INTO projection_family_digests (scope_id, family, digest, record_count, size_bytes, mtime_ns) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&family.scope_id).bind(family.family)
            .bind(shard_digest).bind(i64::try_from(family.record_count)?)
            .bind(size_bytes).bind(mtime_ns)
            .execute(&mut **tx).await?;
    }
    Ok(())
}

/// Hashes the shard file the family's rows were derived from.
///
/// The byte hash is the catch-up comparison baseline; size and mtime are
/// diagnostic metadata and never license a skip. An absent shard hashes as
/// empty bytes, so a later pass that still finds no file sees a match.
fn shard_baseline(
    layout: &ProvenanceLayout,
    family: &FamilyContentDigest,
) -> anyhow::Result<(String, i64, i64)> {
    let scope = if family.scope_id.is_empty() {
        None
    } else {
        Some(ScopeId::new(&family.scope_id)?)
    };
    let path = family.kind.shard_path(layout, scope.as_ref())?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    let (size_bytes, mtime_ns) = match std::fs::metadata(&path) {
        Ok(metadata) => {
            let mtime = metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| {
                    i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
                });
            (i64::try_from(metadata.len())?, mtime)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (0, 0),
        Err(error) => return Err(error.into()),
    };
    Ok((
        crate::canonical_digest::digest(&bytes),
        size_bytes,
        mtime_ns,
    ))
}
