use super::units;
use crate::cache::projection_digest;
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

/// Writes the revision stamp the loaded rows answer under.
///
/// Runs inside the materialization transaction, so rows, the revision, the
/// content digests, and the unit digests commit together or not at all.
/// The instance id is written once from OS entropy and never replaced while
/// the database file lives; it is what keeps serials from different
/// database lifetimes apart. Unit digests hash the SNAPSHOT tree — the same
/// bytes the rows came from — so a live edit racing the rebuild cannot
/// poison them; the next pass sees the difference and re-derives.
pub(super) async fn write_stamp(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    snapshot_layout: &ProvenanceLayout,
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
    for family in &families {
        upsert_content_row(
            tx,
            &family.scope_id,
            family.family,
            &family.digest,
            i64::try_from(family.record_count)?,
        )
        .await?;
    }

    sqlx::query("DELETE FROM projection_unit_digests")
        .execute(&mut **tx)
        .await?;
    crate::test_probes::at("stamp_before_unit_digests")?;
    let state_dir = snapshot_layout.state_dir();
    for unit in units::units_for(scopes) {
        let digest = units::unit_digest(&state_dir, &unit)?;
        upsert_unit_row(tx, &unit.name(), &digest).await?;
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

pub(super) async fn upsert_content_row(
    tx: &mut Transaction<'_, Sqlite>,
    scope_id: &str,
    family: &str,
    content_digest: &str,
    record_count: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO projection_family_digests \
         (scope_id, family, content_digest, record_count) VALUES (?, ?, ?, ?)",
    )
    .bind(scope_id)
    .bind(family)
    .bind(content_digest)
    .bind(record_count)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn upsert_unit_row(
    tx: &mut Transaction<'_, Sqlite>,
    unit: &str,
    digest: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT OR REPLACE INTO projection_unit_digests (unit, digest) VALUES (?, ?)")
        .bind(unit)
        .bind(digest)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
