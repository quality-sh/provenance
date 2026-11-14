//! Catch-up: hash each scope unit and the global unit, then re-derive the
//! changed families by content digest.
//!
//! One pass runs under one guard in one write of the read database. A
//! pass that changes nothing commits no revision.

use super::units::{self, Unit};
use super::{family_rows, relation_rows, stamp, validation};
use crate::cache::{revision_digest_from_stored_rows, ProjectionFamily};
use crate::current_schema::{self, Compatibility};
use crate::publication;
use crate::read_database::{ReadDatabase, Transaction as Lent, WriteWork};
use crate::read_database::{SqlColumns, SqlValue};
#[cfg(feature = "host")]
use crate::repository::Repository;
use provenance_core::ScopeId;
use std::collections::{BTreeMap, BTreeSet};

type ContentKey = (String, String);
type UnitValue = units::UnitDigests;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContentValue {
    content: String,
    stored: String,
    count: i64,
}

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
    pub cache_recreated: bool,
}

#[cfg(feature = "host")]
pub async fn catch_up_state(repository: &Repository) -> anyhow::Result<CatchUpReport> {
    let guard = publication::publication_guard(repository)
        .await?
        .for_pass()?;
    catch_up_with_guard(guard, repository.read_database()?).await
}

/// One catch-up pass in one write of `database`. A pass that succeeds
/// frees `guard` when its work ends, before the store completes its
/// connection, so a writer does not wait on the close. A pass that fails
/// keeps the guard until the connection has completed.
pub async fn catch_up_with_guard(
    guard: publication::PublicationGuard,
    database: &dyn ReadDatabase,
) -> anyhow::Result<CatchUpReport> {
    let mut guard = Some(guard);
    let mut report = None;
    let work = catch_up_work(&mut guard, &mut report, database.max_bound_values());
    let outcome = database.write(work).await;
    drop(guard);
    outcome?;
    report.ok_or_else(|| crate::refusal::fault!("the read database ended a write without its work"))
}

/// The work of one catch-up pass under the guard in `guard`, in a write
/// of the read database. When the pass succeeds, the work frees the guard
/// and puts the report in `report`. When it fails, the guard stays in
/// `guard`, so the caller frees it after the store has completed its
/// connection.
///
/// A statement of the pass binds at most `bound_values` values.
pub fn catch_up_work<'a>(
    guard: &'a mut Option<publication::PublicationGuard>,
    report: &'a mut Option<CatchUpReport>,
    bound_values: usize,
) -> WriteWork<'a> {
    Box::new(move |transaction| {
        Box::pin(async move {
            let held = guard
                .as_ref()
                .ok_or_else(|| crate::refusal::fault!("a catch-up pass runs under the guard"))?;
            *report = Some(pass(held, transaction, bound_values).await?);
            *guard = None;
            Ok(())
        })
    })
}

/// What the rows of a pass found.
enum Step {
    Rebuild,
    Unchanged(CatchUpReport),
    Changed(CatchUpReport),
}

/// One catch-up pass: a rebuild, a pass that changed nothing and so
/// commits nothing, or a pass that commits one revision.
async fn pass(
    guard: &publication::PublicationGuard,
    mut transaction: Lent,
    bound_values: usize,
) -> anyhow::Result<CatchUpReport> {
    let found = step(guard, &mut transaction, bound_values).await?;
    match found {
        Step::Rebuild => rebuild(guard, transaction, bound_values).await,
        Step::Unchanged(report) => Ok(report),
        Step::Changed(report) => {
            crate::test_probes::at("catch_up_before_commit")?;
            transaction.commit().await?;
            crate::test_probes::at("catch_up_after_commit")?;
            Ok(report)
        }
    }
}

/// The rows of one catch-up pass, in the write transaction `tx`.
async fn step(
    guard: &publication::PublicationGuard,
    tx: &mut Lent,
    bound_values: usize,
) -> anyhow::Result<Step> {
    crate::test_probes::at("prepare_current_schema_under_guard")?;
    if current_schema::compatibility(tx).await? == Compatibility::RebuildRequired {
        return Ok(Step::Rebuild);
    }
    let stored = tx
        .fetch(
            "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
            Vec::new(),
        )
        .await?
        .first()
        .map(|row| anyhow::Ok((row.integer(0)?, row.text(1)?.to_owned())))
        .transpose()?;

    // A database with no revision, or one whose schema just moved, is
    // rebuilt under the same guard.
    let Some((stored_serial, stored_digest)) = stored else {
        return Ok(Step::Rebuild);
    };
    if validation::version_changed(tx).await? {
        return Ok(Step::Rebuild);
    }

    let (stored_units, mut content) = load_stored_digests(tx).await?;
    let mut reader = validation::UnitReader::new(guard);
    let (manifest, global_digests) = reader.global(stored_units.get("global"))?;
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
        cache_recreated: false,
    };
    let live = units::units_for(&scope_ids);
    let mut changed = remove_departed_scopes(tx, &stored_units, &live, &mut content).await?;
    if stored_units.get("global") != Some(&global_digests) {
        changed = true;
        stamp::upsert_unit_row(tx, "global", &global_digests).await?;
    }
    for unit in &live {
        let Unit::Scope(scope) = unit else { continue };
        let stored = stored_units.get(&unit.name());
        let Some(bytes) = reader.hash(unit, stored)? else {
            continue;
        };
        changed = true;
        let pass = ScopePass {
            scope,
            stored,
            bytes: &bytes,
            bound_values,
        };
        catch_up_scope(tx, &reader, pass, &mut content, &mut report).await?;
    }
    report.units_hashed = reader.units_hashed();

    if !changed {
        return Ok(Step::Unchanged(report));
    }

    for ((scope_id, family), value) in &content {
        stamp::upsert_content_row(
            tx,
            scope_id,
            family,
            &value.content,
            &value.stored,
            value.count,
        )
        .await?;
    }
    let rows: Vec<(String, String, String, i64)> = content
        .iter()
        .map(|((scope_id, family), value)| {
            (
                scope_id.clone(),
                family.clone(),
                value.content.clone(),
                value.count,
            )
        })
        .collect();
    report.digest = revision_digest_from_stored_rows(&rows)?;
    report.serial = stored_serial + 1;
    stamp::insert_revision(tx, report.serial, &report.digest).await?;
    report.revision_committed = true;
    Ok(Step::Changed(report))
}

async fn load_stored_digests(
    tx: &mut Lent,
) -> anyhow::Result<(
    BTreeMap<String, UnitValue>,
    BTreeMap<ContentKey, ContentValue>,
)> {
    let mut units = BTreeMap::new();
    let unit_rows = tx
        .fetch(
            "SELECT unit, digest, stored_digest FROM projection_unit_digests",
            Vec::new(),
        )
        .await?;
    for mut row in unit_rows {
        let value = UnitValue {
            content: row.take_text(1)?,
            stored: row.take_text(2)?,
        };
        units.insert(row.take_text(0)?, value);
    }
    let mut content = BTreeMap::new();
    let content_rows = tx
        .fetch(
            "SELECT scope_id, family, content_digest, stored_digest, record_count \
             FROM projection_family_digests",
            Vec::new(),
        )
        .await?;
    for mut row in content_rows {
        let value = ContentValue {
            content: row.take_text(2)?,
            stored: row.take_text(3)?,
            count: row.integer(4)?,
        };
        content.insert((row.take_text(0)?, row.take_text(1)?), value);
    }
    Ok((units, content))
}

/// One scope unit whose bytes moved since the stored pair.
struct ScopePass<'a> {
    scope: &'a ScopeId,
    stored: Option<&'a UnitValue>,
    bytes: &'a units::UnitBytes<'a>,
    bound_values: usize,
}

/// Parses the families of one scope unit that its changed files can change,
/// and writes their changed rows. Then stores the digests of the unit and
/// of each of its files.
async fn catch_up_scope(
    tx: &mut Lent,
    reader: &validation::UnitReader<'_>,
    pass: ScopePass<'_>,
    content: &mut BTreeMap<ContentKey, ContentValue>,
    report: &mut CatchUpReport,
) -> anyhow::Result<()> {
    let ScopePass {
        scope,
        stored,
        bytes,
        bound_values,
    } = pass;
    let name = Unit::Scope(scope.clone()).name();
    let read = units::files_read(reader.state_dir(), bytes, &stored_files(tx, &name).await?)?;
    // A unit with no stored pair, or a scope with a family that has no
    // digest row, parses every family, as a rebuild does.
    let all = stored.is_none()
        || ProjectionFamily::ALL.into_iter().any(|family| {
            !content.contains_key(&(scope.as_str().to_owned(), family.family_name().to_owned()))
        });
    let records = reader.scope(scope, &read, all)?;
    rederive_scope(tx, &records, (scope, bound_values), content, report).await?;
    stamp::upsert_unit_row(tx, &name, &read.digests).await?;
    stamp::replace_file_rows(tx, &name, &read.files).await
}

/// The file rows of one unit, by path.
async fn stored_files(
    tx: &mut Lent,
    unit: &str,
) -> anyhow::Result<BTreeMap<String, units::FileDigest>> {
    let rows = tx
        .fetch(
            "SELECT path, stored_digest, content_digest FROM projection_file_digests WHERE unit = ?",
            vec![SqlValue::from(unit)],
        )
        .await?;
    let mut files = BTreeMap::new();
    for mut row in rows {
        let file = units::FileDigest {
            stored: row.take_text(1)?,
            content: row.take_text(2)?,
        };
        files.insert(row.take_text(0)?, file);
    }
    Ok(files)
}

/// Removes the rows, relation rows, and digest rows of every scope the
/// manifest does not name. Returns whether anything departed.
async fn remove_departed_scopes(
    tx: &mut Lent,
    stored_units: &BTreeMap<String, UnitValue>,
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
            tx.execute(
                "DELETE FROM projection_family_digests WHERE scope_id = ?",
                vec![SqlValue::from(scope.as_str())],
            )
            .await?;
        }
        tx.execute(
            "DELETE FROM projection_unit_digests WHERE unit = ?",
            vec![SqlValue::from(name.as_str())],
        )
        .await?;
        tx.execute(
            "DELETE FROM projection_file_digests WHERE unit = ?",
            vec![SqlValue::from(name.as_str())],
        )
        .await?;
        departed = true;
    }
    Ok(departed)
}

/// Rewrites only the parsed families whose content digest moved, and in
/// each of them only the rows that differ. When an owner family moved, the
/// relation rows of its kind follow the same way. A statement binds at
/// most `bound_values` values.
async fn rederive_scope(
    tx: &mut Lent,
    records: &validation::ScopeRecords,
    (scope, bound_values): (&ScopeId, usize),
    content: &mut BTreeMap<ContentKey, ContentValue>,
    report: &mut CatchUpReport,
) -> anyhow::Result<()> {
    let mut moved_owners = Vec::new();
    for records in &records.families {
        let family = records.family;
        let fresh = ContentValue {
            content: family.content_digest(&records.bytes)?,
            stored: crate::canonical_digest::digest(&records.bytes),
            count: i64::try_from(records.count)?,
        };
        let key = (scope.as_str().to_string(), family.family_name().to_string());
        let previous = content.get(&key);
        if previous == Some(&fresh) {
            continue;
        }
        let content_changed = previous.is_none_or(|previous| {
            previous.content != fresh.content || previous.count != fresh.count
        });
        report.rows_written +=
            family_rows::replace_rows(tx, family, scope, &records.bytes, bound_values).await?;
        report.families_rederived += 1;
        if content_changed && relation_rows::OWNERS.contains(&family) {
            if let Some(node_type) = family.node_type() {
                moved_owners.push(crate::cache::serde_name(&node_type)?);
            }
        }
        content.insert(key, fresh);
    }
    if !moved_owners.is_empty() {
        relation_rows::replace_rows(tx, &records.relations, scope, &moved_owners, bound_values)
            .await?;
    }
    Ok(())
}

async fn rebuild(
    guard: &publication::PublicationGuard,
    transaction: Lent,
    bound_values: usize,
) -> anyhow::Result<CatchUpReport> {
    let (report, (serial, digest)) =
        super::rebuild_with_guard(guard, transaction, bound_values).await?;
    Ok(CatchUpReport {
        serial,
        digest,
        rebuilt: true,
        revision_committed: true,
        units_hashed: 0,
        families_rederived: 0,
        rows_written: report.records_loaded,
        cache_recreated: report.cache_recreated,
    })
}
