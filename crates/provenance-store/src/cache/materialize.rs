//! Materialization orchestration.
//!
//! A total rebuild loads every family from canonical bytes and stamps the
//! database with a projection revision: one serial, one digest over every
//! stored family, and the projection instance the serial belongs to. The
//! per-family sweep module computes the digest baseline; the family-load
//! module owns row derivation. Catch-up lives beside this in `catch_up`.

mod binding_records;
mod collaboration_records;
mod family_load;
mod graph_records;
mod sweep;

use super::{open_cache, MaterializeReport};
use crate::cache::projection_families::{family_named, PROJECTION_FAMILIES};
use crate::{layout::ProvenanceLayout, migrations, publication, state_store::StateStore};
use provenance_core::ScopeId;

pub async fn materialize_empty_state(
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    Ok(MaterializeReport {
        records_loaded: 0,
        migrations_applied,
    })
}

pub async fn materialize_state(layout: &ProvenanceLayout) -> anyhow::Result<MaterializeReport> {
    let snapshot = publication::snapshot_state(layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest()?;
    for scope in &manifest.scopes {
        store.validate_ideation_scope(&scope.id)?;
    }
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
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
        records_loaded +=
            family_load::load_family(&mut tx, &store, edges, &manifest.scopes[0].id).await?;
    }

    stamp_revision(&mut tx, snapshot.layout(), &manifest.scopes).await?;
    tx.commit().await?;

    Ok(MaterializeReport {
        records_loaded,
        migrations_applied,
    })
}

/// Writes the projection revision row and the per-family digest baseline.
///
/// The instance identifier is minted once per database and kept across
/// rebuilds; the serial advances by one per committed revision. Serials
/// mean nothing across instances, so every stamp carries both.
async fn stamp_revision(
    tx: &mut sqlx::SqliteConnection,
    layout: &ProvenanceLayout,
    scopes: &[provenance_core::Scope],
) -> anyhow::Result<()> {
    let previous: Option<(i64, String)> =
        sqlx::query_as("SELECT serial, instance_id FROM projection_revision LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?;
    let instance = match &previous {
        Some((_, instance)) => instance.clone(),
        None => sweep::mint_instance_id()?,
    };
    let serial = previous.map_or(1, |(serial, _)| serial + 1);
    let digest = crate::cache::projection_digest::projection_digest(layout)?;
    sqlx::query("DELETE FROM projection_revision")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM projection_family_digests")
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO projection_revision (serial, instance_id, digest) VALUES (?, ?, ?)")
        .bind(serial)
        .bind(&instance)
        .bind(&digest)
        .execute(&mut *tx)
        .await?;
    for family in PROJECTION_FAMILIES {
        if family.global {
            let any_scope = scopes.first().map_or_else(
                || ScopeId::new("default").expect("literal scope id parses"),
                |scope| scope.id.clone(),
            );
            let baseline = sweep::shard_baseline(family, layout, &any_scope);
            write_family_digest(tx, "", &baseline).await?;
        } else {
            for scope in scopes {
                let baseline = sweep::shard_baseline(family, layout, &scope.id);
                write_family_digest(tx, scope.id.as_str(), &baseline).await?;
            }
        }
    }
    Ok(())
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
    .execute(tx)
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
