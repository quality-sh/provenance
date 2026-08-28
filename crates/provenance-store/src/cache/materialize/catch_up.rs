//! Truthful catch-up: the incremental freshness pass.
//!
//! All steps run inside one publication-guard scope. The pass drains the
//! journal for hints, re-derives named families from complete shard
//! bytes, hash-verifies every remaining family against its stored
//! baseline, and commits re-derived rows, the new revision, and the
//! refreshed baselines in one transaction. A family whose bytes hash to
//! the stored digest is never reparsed and its rows are never rewritten;
//! total reload stays bootstrap and repair only.
//!
//! Truth rules: a response may claim full freshness only when the pass
//! that produced the stamp read and hashed the complete canonical bytes
//! of every stored (scope, family) behind it. Size and mtime never
//! license a skip. The journal is a hint, never proof.

use super::family_load;
use super::sweep::{shard_baseline, FamilyBaseline};
use crate::cache::open_cache;
use crate::cache::projection_families::ProjectionFamily;
use crate::cache::projection_families::PROJECTION_FAMILIES;
use crate::layout::ProvenanceLayout;
use crate::{migrations, publication::journal};
use provenance_core::{Manifest, ScopeId};
use serde::Serialize;
use sqlx::{Pool, Row, Sqlite};

/// What one catch-up pass did.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatchUpReport {
    pub rebuilt: bool,
    pub families_rederived: Vec<String>,
    pub families_verified: usize,
    pub journal_drained: usize,
    pub serial: i64,
    pub digest: String,
    pub instance_id: String,
    pub migrations_applied: Vec<String>,
}

/// The stored revision, absent for a never-materialized database.
struct StoredRevision {
    serial: i64,
    instance_id: String,
}

async fn read_revision(pool: &Pool<Sqlite>) -> anyhow::Result<Option<StoredRevision>> {
    let row = sqlx::query("SELECT serial, instance_id, digest FROM projection_revision LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| StoredRevision {
        serial: row.get("serial"),
        instance_id: row.get("instance_id"),
    }))
}

/// Brings the projection behind `layout` current with canonical bytes and
/// returns the stamp the pass committed.
pub async fn catch_up_state(layout: &ProvenanceLayout) -> anyhow::Result<CatchUpReport> {
    let guard = crate::publication::guard::publication_guard(layout).await?;
    catch_up_state_under_guard(&guard, layout).await
}

/// The guarded variant: the caller must hold the publication guard.
pub async fn catch_up_state_under_guard(
    guard: &crate::publication::guard::PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<CatchUpReport> {
    let store = crate::state_store::StateStore::new(layout.clone());
    let manifest = store.manifest_under_guard(guard)?;

    let pool = open_cache(layout).await?;

    // Step 1. Schema first: a fresh database has no revision to read, and
    // a changed schema forces a total rebuild before any serve.
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    let stored = read_revision(&pool).await?;
    let stored_serial = stored
        .as_ref()
        .map_or(0, |revision| revision.serial.max(0).cast_unsigned());

    // Step 0. Guard held; normalize the serial head before any allocation
    // or coverage calculation.
    let head = journal::normalize_head(layout, stored_serial)?;
    if stored.is_none() || !migrations_applied.is_empty() {
        let report = super::materialize_state_under_guard(guard, layout).await?;
        journal::normalize_head(layout, report.serial.max(0).cast_unsigned())?;
        journal::prune_through(layout, report.serial.max(0).cast_unsigned())?;
        return Ok(CatchUpReport {
            rebuilt: true,
            families_rederived: PROJECTION_FAMILIES
                .iter()
                .map(|family| family.name.to_string())
                .collect(),
            families_verified: 0,
            journal_drained: 0,
            serial: report.serial,
            digest: report.digest,
            instance_id: report.instance_id,
            migrations_applied,
        });
    }
    let stored = stored.expect("revision presence checked above");

    // Step 2. Drain journal hints in stored_serial + 1 ..= head - 1.
    let drained = journal::drain_window(layout, stored_serial, head)?;
    let mut rederived: Vec<(String, String)> = Vec::new();
    for event in &drained {
        let pair = (event.family.clone(), event.scope.clone());
        if !rederived.contains(&pair) {
            rederived.push(pair);
        }
    }

    // Steps 2 and 3. Hash every stored (scope, family); re-derive the
    // hinted ones and the ones whose bytes differ from the baseline.
    let mut verified = 0usize;
    let mut baselines: Vec<(String, String, FamilyBaseline)> = Vec::new();
    let mut work: Vec<(&'static ProjectionFamily, Option<ScopeId>)> = Vec::new();
    for family in PROJECTION_FAMILIES {
        for scope in family_scope_ids(family, &manifest) {
            let key_scope = scope
                .as_ref()
                .map_or(String::new(), |scope| scope.as_str().to_string());
            let hinted = rederived
                .iter()
                .any(|(name, hint_scope)| *name == family.name && hint_scope == &key_scope);
            let baseline = shard_baseline(family, layout, scope.as_ref());
            let stored_digest = stored_family_digest(&pool, &key_scope, family.name).await?;
            if hinted || stored_digest.as_deref() != Some(baseline.digest.as_str()) {
                work.push((family, scope));
            } else {
                verified += 1;
            }
            baselines.push((family.name.to_string(), key_scope, baseline));
        }
    }

    // Step 4. Commit rows, the new revision, and the baselines together.
    let serial = i64::try_from(head).unwrap_or(i64::MAX);
    eprintln!("DBG before-digest");
    let digest = crate::cache::projection_digest::projection_digest(layout, &manifest)?;
    eprintln!("DBG after-digest");
    eprintln!("DBG before-begin");
    let mut tx = pool.begin().await?;
    eprintln!("DBG begin-ok");
    for (family, scope) in &work {
        rederive_family(&mut tx, &store, family, scope.as_ref(), &manifest).await?;
    }
    for (family, scope, baseline) in &baselines {
        write_family_digest(&mut tx, scope, family, baseline).await?;
    }
    write_revision(&mut tx, serial, &stored.instance_id, &digest).await?;
    tx.commit().await?;
    journal::normalize_head(layout, head)?;
    journal::prune_through(layout, head.saturating_sub(1))?;
    pool.close().await;

    Ok(CatchUpReport {
        rebuilt: false,
        families_rederived: work
            .iter()
            .map(|(family, _)| family.name.to_string())
            .collect(),
        families_verified: verified,
        journal_drained: drained.len(),
        serial,
        digest,
        instance_id: stored.instance_id,
        migrations_applied,
    })
}

fn family_scope_ids(family: &ProjectionFamily, manifest: &Manifest) -> Vec<Option<ScopeId>> {
    if family.global {
        vec![None]
    } else {
        manifest
            .scopes
            .iter()
            .map(|scope| Some(scope.id.clone()))
            .collect()
    }
}

async fn stored_family_digest(
    pool: &Pool<Sqlite>,
    scope_id: &str,
    family: &str,
) -> anyhow::Result<Option<String>> {
    let row = sqlx::query(
        "SELECT digest FROM projection_family_digests WHERE scope_id = ? AND family = ?",
    )
    .bind(scope_id)
    .bind(family)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| row.get::<String, _>("digest")))
}

/// Re-derives one family's rows from the canonical readers, inside the
/// pass's transaction. Row content always comes from canonical bytes, so
/// a phantom event can never inject a row canonical does not hold.
async fn rederive_family(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    store: &crate::state_store::StateStore,
    family: &ProjectionFamily,
    scope: Option<&ScopeId>,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    if let Some(scope) = scope {
        family_load::clear_family_scope(tx, family, scope).await?;
        family_load::load_family(tx, store, family, scope).await?;
    } else {
        // A global family is one shard covering every scope, so the rows
        // are replaced whole.
        family_load::clear_family(tx, family).await?;
        let any_scope = manifest.scopes.first().map_or_else(
            || ScopeId::new("default").expect("literal scope id parses"),
            |scope| scope.id.clone(),
        );
        family_load::load_family(tx, store, family, &any_scope).await?;
    }
    Ok(())
}

async fn write_family_digest(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    scope_id: &str,
    family: &str,
    baseline: &FamilyBaseline,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO projection_family_digests (scope_id, family, digest, record_count, size_bytes, mtime_ns) VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT (scope_id, family) DO UPDATE SET digest = excluded.digest, record_count = excluded.record_count, size_bytes = excluded.size_bytes, mtime_ns = excluded.mtime_ns",
    )
    .bind(scope_id)
    .bind(family)
    .bind(&baseline.digest)
    .bind(baseline.record_count)
    .bind(baseline.size_bytes)
    .bind(baseline.mtime_ns)
    .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn write_revision(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    serial: i64,
    instance_id: &str,
    digest: &str,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM projection_revision")
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO projection_revision (serial, instance_id, digest) VALUES (?, ?, ?)")
        .bind(serial)
        .bind(instance_id)
        .bind(digest)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
