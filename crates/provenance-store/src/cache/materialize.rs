//! Materialization orchestration.
//!
//! A total rebuild loads every family from canonical bytes and stamps the
//! database with a projection revision: one serial, one digest over every
//! stored family, and the projection instance the serial belongs to. The
//! per-family sweep module computes the digest baseline; the family-load
//! module owns row derivation. Catch-up lives beside this in `catch_up`.
//!
//! Every write to the projection database, including total rebuild, holds
//! the repository publication guard across migrations and the row
//! transaction. The guard is acquired once here and passed as an explicit
//! capability to the locked helper variants.

mod binding_records;
mod catch_up;
mod collaboration_records;
mod family_load;
mod graph_records;
mod sweep;

use super::{open_cache, MaterializeReport};
use crate::cache::projection_families::family_named;
use crate::cache::projection_families::PROJECTION_FAMILIES;
use crate::layout::ProvenanceLayout;
use crate::{migrations, publication, state_store::StateStore};
use provenance_core::ScopeId;
use sqlx::Row;

pub async fn materialize_empty_state(
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    Ok(MaterializeReport {
        records_loaded: 0,
        migrations_applied,
        serial: 0,
        digest: String::new(),
        instance_id: String::new(),
    })
}

pub use catch_up::{catch_up_state, catch_up_state_under_guard, CatchUpReport};

pub async fn materialize_state(layout: &ProvenanceLayout) -> anyhow::Result<MaterializeReport> {
    let guard = publication::guard::publication_guard(layout).await?;
    materialize_state_under_guard(&guard, layout).await
}

/// The guarded rebuild: the caller must hold the publication guard.
pub(super) async fn materialize_state_under_guard(
    guard: &publication::guard::PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let snapshot = publication::guard::snapshot_state_under_guard(guard, layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest_under_guard(guard)?;
    for scope in &manifest.scopes {
        store.validate_ideation_scope_under_guard(guard, &scope.id)?;
    }
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    let stored_serial = stored_revision_serial(&pool).await?;
    let serial: i64 = i64::try_from(publication::journal::normalize_head(layout, stored_serial)?)
        .unwrap_or(i64::MAX);
    let mut tx = pool.begin().await?;
    for family in PROJECTION_FAMILIES {
        family_load::clear_family(&mut tx, family).await?;
    }

    let mut records_loaded = 0;
    for scope in &manifest.scopes {
        for family in PROJECTION_FAMILIES {
            if family.global {
                continue;
            }
            records_loaded += family_load::load_family(&mut tx, &store, family, &scope.id).await?;
        }
    }
    if let Some(edges) = family_named("edges") {
        let any_scope = manifest.scopes.first().map_or_else(
            || ScopeId::new("default").expect("literal scope id parses"),
            |scope| scope.id.clone(),
        );
        records_loaded += family_load::load_family(&mut tx, &store, edges, &any_scope).await?;
    }

    let digest = crate::cache::projection_digest::projection_digest(snapshot.layout(), &manifest)?;
    let instance_id = stamp_revision(&mut tx, layout, serial, &digest, &manifest).await?;
    tx.commit().await?;

    let committed = serial.max(0).cast_unsigned();
    publication::journal::normalize_head(layout, committed)?;
    publication::journal::prune_through(layout, committed.saturating_sub(1))?;

    Ok(MaterializeReport {
        records_loaded,
        migrations_applied,
        serial,
        digest,
        instance_id,
    })
}

async fn stored_revision_serial(pool: &sqlx::Pool<sqlx::Sqlite>) -> anyhow::Result<u64> {
    let row = sqlx::query("SELECT serial FROM projection_revision LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map_or(0, |row| {
        let serial: i64 = row.get("serial");
        u64::try_from(serial.clamp(0, i64::from(u32::MAX))).unwrap_or(u64::MAX)
    }))
}

/// Writes the projection revision row and the per-family digest baseline.
///
/// The instance identifier is minted once per database and kept across
/// rebuilds; serials mean nothing across instances, so every stamp
/// carries both.
async fn stamp_revision(
    tx: &mut sqlx::SqliteConnection,
    layout: &ProvenanceLayout,
    serial: i64,
    digest: &str,
    manifest: &provenance_core::Manifest,
) -> anyhow::Result<String> {
    let instance_id: String =
        sqlx::query_scalar("SELECT instance_id FROM projection_revision LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(|| {
                sweep::mint_instance_id().expect("instance id entropy is available")
            });
    sqlx::query("DELETE FROM projection_revision")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM projection_family_digests")
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO projection_revision (serial, instance_id, digest) VALUES (?, ?, ?)")
        .bind(serial)
        .bind(&instance_id)
        .bind(digest)
        .execute(&mut *tx)
        .await?;
    for family in PROJECTION_FAMILIES {
        if family.global {
            let baseline = sweep::shard_baseline(family, layout, None);
            write_family_digest(tx, "", &baseline).await?;
        } else {
            for scope in &manifest.scopes {
                let baseline = sweep::shard_baseline(family, layout, Some(&scope.id));
                write_family_digest(tx, scope.id.as_str(), &baseline).await?;
            }
        }
    }
    Ok(instance_id)
}

async fn write_family_digest(
    tx: &mut sqlx::SqliteConnection,
    scope_id: &str,
    baseline: &sweep::FamilyBaseline,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO projection_family_digests (scope_id, family, digest, record_count, size_bytes, mtime_ns) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(scope_id)
    .bind(baseline.family)
    .bind(&baseline.digest)
    .bind(baseline.record_count)
    .bind(baseline.size_bytes)
    .bind(baseline.mtime_ns)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod loader_coverage {
    use super::*;

    /// Every family in `PROJECTION_FAMILIES` must have a row loader; a
    /// family added to the table without one cannot be stored and the
    /// stamp would silently miss it.
    #[test]
    fn every_family_has_a_row_loader() {
        let names = family_load::loader_names();
        for family in PROJECTION_FAMILIES {
            assert!(
                names.contains(&family.name),
                "projection family '{}' has no row loader",
                family.name
            );
        }
        assert_eq!(names.len(), PROJECTION_FAMILIES.len());
    }

    #[test]
    fn family_table_stores_nineteen_families() {
        assert_eq!(PROJECTION_FAMILIES.len(), 19);
    }
}
