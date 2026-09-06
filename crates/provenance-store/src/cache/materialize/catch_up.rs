//! Catch-up: hash each scope unit and the global unit, then re-derive the
//! changed families by content digest.
//!
//! One pass runs under one guard in one transaction. A pass that changes
//! nothing commits no revision.

use super::units::{self, Unit};
use super::{family_rows, relation_rows, stamp, validation};
use crate::cache::{open_cache, revision_digest_from_stored_rows, ProjectionFamily};
use crate::{canonical_digest, layout::ProvenanceLayout, migrations, publication};
use provenance_core::ScopeId;
use std::collections::{BTreeMap, BTreeSet};

type ContentKey = (String, String);
type ContentValue = (String, i64);

/// What one catch-up pass did.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CatchUpReport {
    pub serial: i64,
    pub digest: String,
    pub rebuilt: bool,
    pub revision_committed: bool,
    /// Units scanned. Verification hashes do not add units.
    pub units_hashed: u64,
    pub families_rederived: u64,
    pub rows_written: u64,
    pub migrations_applied: Vec<String>,
}

pub async fn catch_up_state(layout: &ProvenanceLayout) -> anyhow::Result<CatchUpReport> {
    let guard = publication::publication_guard(layout).await?;
    let pool = open_cache(layout).await?;
    let report = catch_up_with_guard(&guard, &pool, layout).await;
    // Close rather than drop. A dropped pool releases its file handles
    // asynchronously, and on Windows a later delete of the database file
    // races that release.
    crate::cache::close_cache(&pool).await;
    report
}

/// One catch-up pass on the caller's pool, for a caller that holds the
/// guard. The pool stays open for the caller.
pub async fn catch_up_with_guard(
    guard: &publication::PublicationGuard,
    pool: &sqlx::SqlitePool,
    layout: &ProvenanceLayout,
) -> anyhow::Result<CatchUpReport> {
    crate::test_probes::at("run_migrations_under_guard")?;
    let migrations_applied = migrations::run_migrations(pool, layout).await?;
    crate::test_probes::at("catch_up_after_migrations")?;
    let stored: Option<(i64, String)> = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    // A database with no revision, or one whose schema just moved, is
    // rebuilt under the same guard.
    let Some((stored_serial, stored_digest)) = stored else {
        return rebuild(guard, layout, pool, migrations_applied).await;
    };
    if !migrations_applied.is_empty() || validation::version_changed(pool).await? {
        return rebuild(guard, layout, pool, migrations_applied).await;
    }

    let (stored_units, mut content) = load_stored_digests(pool).await?;
    let mut reader = validation::UnitReader::new(guard);
    let (manifest, global_digest) = reader.global(stored_units.get("global"))?;
    let scope_ids: Vec<ScopeId> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    let mut report = CatchUpReport {
        serial: stored_serial,
        digest: stored_digest,
        rebuilt: false,
        revision_committed: false,
        units_hashed: 0,
        families_rederived: 0,
        rows_written: 0,
        migrations_applied,
    };
    let mut tx = pool.begin().await?;

    let live = units::units_for(&scope_ids);
    let mut changed = remove_departed_scopes(&mut tx, &stored_units, &live, &mut content).await?;
    if stored_units.get("global") != Some(&global_digest) {
        changed = true;
        stamp::upsert_unit_row(&mut tx, "global", &global_digest).await?;
    }
    for unit in &live {
        let Unit::Scope(scope) = unit else { continue };
        let digest = reader.hash(unit)?;
        if stored_units.get(&unit.name()) == Some(&digest) {
            continue;
        }
        let (records, digest) = reader.scope(scope, digest)?;
        changed = true;
        rederive_scope(&mut tx, &records, scope, &mut content, &mut report).await?;
        stamp::upsert_unit_row(&mut tx, &unit.name(), &digest).await?;
    }
    report.units_hashed = reader.units_hashed;

    if !changed {
        drop(tx);
        return Ok(report);
    }

    for ((scope_id, family), (digest, count)) in &content {
        stamp::upsert_content_row(&mut tx, scope_id, family, digest, *count).await?;
    }
    let rows: Vec<(String, String, String, i64)> = content
        .iter()
        .map(|((scope_id, family), (digest, count))| {
            (scope_id.clone(), family.clone(), digest.clone(), *count)
        })
        .collect();
    report.digest = revision_digest_from_stored_rows(&rows)?;
    report.serial = stored_serial + 1;
    stamp::insert_revision(&mut tx, report.serial, &report.digest).await?;
    report.revision_committed = true;
    crate::test_probes::at("catch_up_before_commit")?;
    tx.commit().await?;
    crate::test_probes::at("catch_up_after_commit")?;
    Ok(report)
}

async fn load_stored_digests(
    pool: &sqlx::SqlitePool,
) -> anyhow::Result<(BTreeMap<String, String>, BTreeMap<ContentKey, ContentValue>)> {
    let units: BTreeMap<String, String> =
        sqlx::query_as::<_, (String, String)>("SELECT unit, digest FROM projection_unit_digests")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let content: BTreeMap<ContentKey, ContentValue> =
        sqlx::query_as::<_, (String, String, String, i64)>(
            "SELECT scope_id, family, content_digest, record_count FROM projection_family_digests",
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(scope_id, family, digest, count)| ((scope_id, family), (digest, count)))
        .collect();
    Ok((units, content))
}

/// Removes the rows, relation rows, and digest rows of every scope the
/// manifest does not name. Returns whether anything departed.
async fn remove_departed_scopes(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    stored_units: &BTreeMap<String, String>,
    live: &[Unit],
    content: &mut BTreeMap<ContentKey, ContentValue>,
) -> anyhow::Result<bool> {
    let live_names: BTreeSet<String> = live.iter().map(Unit::name).collect();
    let mut departed = false;
    for name in stored_units.keys() {
        if live_names.contains(name) {
            continue;
        }
        if let Some(scope) = Unit::scope_of(name)? {
            for family in ProjectionFamily::ALL {
                family_rows::delete_rows(tx, family, &scope).await?;
            }
            relation_rows::delete_rows(tx, &scope).await?;
            content.retain(|(scope_id, _), _| scope_id != scope.as_str());
            sqlx::query("DELETE FROM projection_family_digests WHERE scope_id = ?")
                .bind(scope.as_str())
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query("DELETE FROM projection_unit_digests WHERE unit = ?")
            .bind(name)
            .execute(&mut **tx)
            .await?;
        departed = true;
    }
    Ok(departed)
}

/// The families whose records declare relations.
const RELATION_OWNERS: [ProjectionFamily; 7] = [
    ProjectionFamily::Sources,
    ProjectionFamily::Requirements,
    ProjectionFamily::Resolutions,
    ProjectionFamily::Rules,
    ProjectionFamily::Topics,
    ProjectionFamily::Questions,
    ProjectionFamily::Boundaries,
];

/// Rewrites only the parsed families
/// whose content digest moved; the scope's relation rows follow whenever
/// an owner family did.
async fn rederive_scope(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    records: &validation::ScopeRecords,
    scope: &ScopeId,
    content: &mut BTreeMap<ContentKey, ContentValue>,
    report: &mut CatchUpReport,
) -> anyhow::Result<()> {
    let mut owner_moved = false;
    for records in &records.families {
        let family = records.family;
        let fresh = (
            canonical_digest::digest(&records.bytes),
            i64::try_from(records.count)?,
        );
        let key = (scope.as_str().to_string(), family.family_name().to_string());
        if content.get(&key) == Some(&fresh) {
            continue;
        }
        family_rows::delete_rows(tx, family, scope).await?;
        report.rows_written += family_rows::load_rows(tx, family, &records.bytes).await?;
        report.families_rederived += 1;
        owner_moved |= RELATION_OWNERS.contains(&family);
        content.insert(key, fresh);
    }
    if owner_moved {
        relation_rows::delete_rows(tx, scope).await?;
        relation_rows::load_rows(tx, &records.relations, scope).await?;
    }
    Ok(())
}

async fn rebuild(
    guard: &publication::PublicationGuard,
    layout: &ProvenanceLayout,
    pool: &sqlx::SqlitePool,
    migrations_applied: Vec<String>,
) -> anyhow::Result<CatchUpReport> {
    let report = super::materialize_with_guard(guard, layout).await?;
    let (serial, digest): (i64, String) = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await?;
    Ok(CatchUpReport {
        serial,
        digest,
        rebuilt: true,
        revision_committed: true,
        units_hashed: 0,
        families_rederived: 0,
        rows_written: report.records_loaded,
        migrations_applied,
    })
}
