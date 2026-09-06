mod catch_up;
mod collaboration_records;
mod family_rows;
mod record_rows;
mod relation_rows;
mod stamp;
mod units;
mod validation;

pub use catch_up::catch_up_with_guard;
pub use catch_up::{catch_up_state, CatchUpReport};
pub use record_rows::SEARCH_TEXT;
pub use units::{unit_digest, units_for, Unit};

use super::{open_cache, MaterializeReport};
use crate::{layout::ProvenanceLayout, migrations, publication};
use sqlx::{Sqlite, Transaction};

pub async fn materialize_empty_state(
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let guard = publication::publication_guard(layout).await?;
    let _held = &guard;
    crate::test_probes::at("materialize_empty_under_guard")?;
    let pool = open_cache(layout).await?;
    crate::test_probes::at("run_migrations_under_guard")?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    // Close rather than drop. A dropped pool releases its file handles
    // asynchronously, and on Windows a later delete of the database file
    // races that release.
    pool.close().await;
    Ok(MaterializeReport {
        records_loaded: 0,
        migrations_applied,
    })
}

pub async fn materialize_state(layout: &ProvenanceLayout) -> anyhow::Result<MaterializeReport> {
    let guard = publication::publication_guard(layout).await?;
    materialize_with_guard(&guard, layout).await
}

/// The rebuild body for a caller that holds the guard.
///
/// Hashing, validation, migrations, and the commit all run under the
/// guard. The serial is the stored serial plus one.
pub(super) async fn materialize_with_guard(
    guard: &publication::PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let mut reader = validation::UnitReader::new(guard);
    let (manifest, global_digest) = reader.global(None)?;
    let pool = open_cache(layout).await?;
    crate::test_probes::at("run_migrations_under_guard")?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    let stored_serial: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(serial), 0) FROM projection_revision")
            .fetch_one(&pool)
            .await?;
    let mut tx = pool.begin().await?;
    clear_cache(&mut tx).await?;

    stamp::upsert_unit_row(&mut tx, "global", &global_digest).await?;
    let mut records_loaded = 0;
    let scopes: Vec<_> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    for unit in units::units_for(&scopes) {
        let Unit::Scope(scope) = &unit else { continue };
        let digest = reader.hash(&unit)?;
        let (records, digest) = reader.scope(scope, digest)?;
        for records in &records.families {
            records_loaded +=
                family_rows::load_rows(&mut tx, records.family, &records.bytes).await?;
            stamp::upsert_content_row(
                &mut tx,
                scope.as_str(),
                records.family.family_name(),
                &crate::canonical_digest::digest(&records.bytes),
                i64::try_from(records.count)?,
            )
            .await?;
        }
        relation_rows::load_rows(&mut tx, &records.relations, scope).await?;
        stamp::upsert_unit_row(&mut tx, &unit.name(), &digest).await?;
    }
    stamp::write_stamp(&mut tx, stored_serial + 1).await?;
    crate::test_probes::at("materialize_before_commit")?;
    tx.commit().await?;
    crate::test_probes::at("materialize_after_commit")?;
    pool.close().await;

    Ok(MaterializeReport {
        records_loaded,
        migrations_applied,
    })
}

async fn clear_cache(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    for table in [
        "relations",
        "projection_family_digests",
        "projection_unit_digests",
    ] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut **tx)
            .await?;
    }
    for family in super::ProjectionFamily::ALL {
        sqlx::query(&format!("DELETE FROM {}", family.family_name()))
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
