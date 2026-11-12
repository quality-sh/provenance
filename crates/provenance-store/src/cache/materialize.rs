mod batch;
mod catch_up;
mod collaboration_records;
mod family_inputs;
mod family_rows;
mod record_rows;
mod relation_rows;
mod row_changes;
mod stamp;
mod units;
mod validation;

pub use catch_up::catch_up_with_guard;
pub(crate) use catch_up::catch_up_work;
pub use catch_up::{catch_up_state, CatchUpReport};
pub use record_rows::SEARCH_TEXT;
pub use units::unit_stored_digest;
pub use units::{scope_ids, unit_digest, units_for, Unit, UnitHashError};

use super::MaterializeReport;
use crate::current_schema::{self, Compatibility};
use crate::publication;
use crate::read_database::file::FileSqlx as _;
use crate::read_database::{ReadDatabase, Transaction as Lent};
use crate::repository::Repository;
use provenance_macros::rule;
use sqlx::{Sqlite, Transaction};

/// The `SQLx` transaction of the file adapter, which the projection writes
/// through.
pub(super) fn file_transaction(
    transaction: &mut Lent,
) -> anyhow::Result<&mut Transaction<'static, Sqlite>> {
    transaction
        .sqlx()
        .ok_or_else(|| crate::refusal::fault!("the projection writes the file adapter"))
}

pub async fn materialize_empty_state(repository: &Repository) -> anyhow::Result<MaterializeReport> {
    let guard = publication::publication_guard(repository).await?;
    let _held = &guard;
    crate::test_probes::at("materialize_empty_under_guard")?;
    // Everything in the write is part of the settled work: a failure there
    // completes the connection before it reports.
    let mut cache_recreated = false;
    let slot = &mut cache_recreated;
    repository
        .read_database()?
        .write(Box::new(move |transaction| {
            Box::pin(async move {
                *slot = initialize_empty(transaction).await?;
                Ok(())
            })
        }))
        .await?;
    Ok(MaterializeReport {
        records_loaded: 0,
        cache_recreated,
    })
}

async fn initialize_empty(mut transaction: Lent) -> anyhow::Result<bool> {
    crate::test_probes::at("prepare_current_schema_under_guard")?;
    let tx = file_transaction(&mut transaction)?;
    let recreate = current_schema::compatibility(tx).await? == Compatibility::RebuildRequired;
    if !recreate {
        return Ok(false);
    }
    current_schema::replace(tx).await?;
    current_schema::mark_current(tx).await?;
    transaction.commit().await?;
    Ok(true)
}

pub async fn materialize_state(repository: &Repository) -> anyhow::Result<MaterializeReport> {
    let guard = publication::publication_guard(repository).await?;
    materialize_with_guard(&guard, repository.read_database()?).await
}

/// The rebuild body for a caller that holds the guard.
///
/// Hashing, validation, schema preparation, and the commit all run under the
/// guard. The serial is the stored serial plus one. The connection is
/// complete before the report or the failure leaves, however the body
/// ends.
pub(super) async fn materialize_with_guard(
    guard: &publication::PublicationGuard,
    database: &dyn ReadDatabase,
) -> anyhow::Result<MaterializeReport> {
    let mut reader = validation::UnitReader::new(guard);
    let (manifest, global_digests) = reader.global(None)?;
    let mut report = None;
    let (slot, reader) = (&mut report, &reader);
    database
        .write(Box::new(move |transaction| {
            Box::pin(async move {
                crate::test_probes::at("prepare_current_schema_under_guard")?;
                let (rebuilt, _) =
                    rebuild_rows(transaction, reader, manifest, global_digests).await?;
                *slot = Some(rebuilt);
                Ok(())
            })
        }))
        .await?;
    report.ok_or_else(|| crate::refusal::fault!("the read database ended a write without its work"))
}

/// Rebuilds in the caller's write transaction and commits it. Gives the
/// report and the committed revision.
pub(super) async fn rebuild_with_guard(
    guard: &publication::PublicationGuard,
    transaction: Lent,
) -> anyhow::Result<(MaterializeReport, (i64, String))> {
    let mut reader = validation::UnitReader::new(guard);
    let (manifest, global_digests) = reader.global(None)?;
    rebuild_rows(transaction, &reader, manifest, global_digests).await
}

/// Hashes, validates, reloads, and commits one full rebuild in the
/// caller's write transaction. Gives the report and the committed
/// revision.
#[rule("rule_interrupted_migration_reloads_every_family")]
async fn rebuild_rows(
    mut transaction: Lent,
    reader: &validation::UnitReader<'_>,
    manifest: provenance_core::Manifest,
    global_digests: units::UnitDigests,
) -> anyhow::Result<(MaterializeReport, (i64, String))> {
    let tx = file_transaction(&mut transaction)?;
    let cache_recreated =
        current_schema::compatibility(tx).await? == Compatibility::RebuildRequired;
    if cache_recreated {
        current_schema::replace(tx).await?;
    } else {
        clear_cache(tx).await?;
    }
    let stored_serial: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(serial), 0) FROM projection_revision")
            .fetch_one(&mut **tx)
            .await?;

    stamp::upsert_unit_row(tx, "global", &global_digests).await?;
    let mut records_loaded = 0;
    let scopes: Vec<_> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    for unit in units::units_for(&scopes) {
        let Unit::Scope(scope) = &unit else { continue };
        let bytes = reader
            .hash(&unit, None)?
            .expect("a unit with no stored pair has changed");
        let read = units::files_read(
            reader.state_dir(),
            &bytes,
            &std::collections::BTreeMap::new(),
        )?;
        let records = reader.scope(scope, &read, true)?;
        for records in &records.families {
            records_loaded += family_rows::load_rows(tx, records.family, &records.bytes).await?;
            stamp::upsert_content_row(
                tx,
                scope.as_str(),
                records.family.family_name(),
                &records.family.content_digest(&records.bytes)?,
                &crate::canonical_digest::digest(&records.bytes),
                i64::try_from(records.count)?,
            )
            .await?;
        }
        relation_rows::load_rows(tx, &records.relations, scope).await?;
        stamp::upsert_unit_row(tx, &unit.name(), &read.digests).await?;
        stamp::replace_file_rows(tx, &unit.name(), &read.files).await?;
    }
    stamp::write_stamp(tx, stored_serial + 1).await?;
    if cache_recreated {
        current_schema::mark_current(tx).await?;
    }
    crate::test_probes::at("materialize_before_commit")?;
    let revision: (i64, String) = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_one(&mut **tx)
    .await?;
    transaction.commit().await?;
    crate::test_probes::at("materialize_after_commit")?;

    Ok((
        MaterializeReport {
            records_loaded,
            cache_recreated,
        },
        revision,
    ))
}

async fn clear_cache(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    for table in [
        "relations",
        "record_identities",
        "projection_family_digests",
        "projection_unit_digests",
        "projection_file_digests",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table}")))
            .execute(&mut **tx)
            .await?;
    }
    for family in super::ProjectionFamily::ALL {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {}",
            family.family_name()
        )))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
