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
    snapshot_layout: &ProvenanceLayout,
    layout: &ProvenanceLayout,
    scopes: &[ScopeId],
    serial: i64,
) -> anyhow::Result<()> {
    let families = projection_digest::family_content_digests(store, scopes)?;
    let revision = projection_digest::revision_digest(&families)?;

    sqlx::query("INSERT OR IGNORE INTO projection_instance (only_row, instance_id) VALUES (1, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&mut **tx)
        .await?;

    insert_revision(tx, serial, &revision).await?;

    sqlx::query("DELETE FROM projection_family_digests")
        .execute(&mut **tx)
        .await?;
    crate::test_probes::at("stamp_before_baselines")?;
    for family in &families {
        let (shard_digest, size_bytes, mtime_ns) = shard_baseline(snapshot_layout, layout, family)?;
        insert_family_digest_row(
            tx,
            &family.scope_id,
            family.family,
            &shard_digest,
            &family.digest,
            i64::try_from(family.record_count)?,
            size_bytes,
            mtime_ns,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn insert_revision(
    tx: &mut Transaction<'_, Sqlite>,
    serial: i64,
    digest: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO projection_revision (serial, digest) VALUES (?, ?)")
        .bind(serial)
        .bind(digest)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[expect(clippy::too_many_arguments, reason = "one row, one column each")]
pub(super) async fn insert_family_digest_row(
    tx: &mut Transaction<'_, Sqlite>,
    scope_id: &str,
    family: &str,
    shard_digest: &str,
    content_digest: &str,
    record_count: i64,
    size_bytes: i64,
    mtime_ns: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO projection_family_digests \
         (scope_id, family, digest, content_digest, record_count, size_bytes, mtime_ns) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(scope_id)
    .bind(family)
    .bind(shard_digest)
    .bind(content_digest)
    .bind(record_count)
    .bind(size_bytes)
    .bind(mtime_ns)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Hashes the byte domain the family's rows were derived from.
///
/// The hash reads the SNAPSHOT tree — the same bytes the rows came from —
/// so a live edit racing the rebuild cannot poison the baseline; the next
/// pass sees the difference and re-derives. Size and mtime come from the
/// live tree and are diagnostic metadata only; they never license a skip.
fn shard_baseline(
    snapshot_layout: &ProvenanceLayout,
    layout: &ProvenanceLayout,
    family: &FamilyContentDigest,
) -> anyhow::Result<(String, i64, i64)> {
    let scope = if family.scope_id.is_empty() {
        None
    } else {
        Some(ScopeId::new(&family.scope_id)?)
    };
    let domain = family.kind.byte_domain(snapshot_layout, scope.as_ref())?;
    let bytes = crate::cache::domain_bytes(&domain)?;
    let (size_bytes, mtime_ns) =
        crate::cache::domain_metadata(&family.kind.byte_domain(layout, scope.as_ref())?)?;
    Ok((
        crate::canonical_digest::digest(&bytes),
        size_bytes,
        mtime_ns,
    ))
}
