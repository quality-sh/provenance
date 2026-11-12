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

#[cfg(test)]
#[cfg(feature = "host")]
pub use catch_up::catch_up_with_guard;
#[cfg(feature = "host")]
pub use catch_up::catch_up_work;
pub use catch_up::{catch_up_state, CatchUpReport};
pub use record_rows::SEARCH_TEXT;
#[cfg(feature = "host")]
pub use units::unit_stored_digest;
pub use units::{scope_ids, unit_digest, units_for, Unit, UnitHashError};
#[cfg(feature = "host")]
pub use validation::stored_version as stored_validation_version;

use super::MaterializeReport;
use crate::current_schema::{self, Compatibility};
use crate::publication;
use crate::read_database::{ReadDatabase, SqlColumns, Transaction as Lent};
use crate::repository::Repository;
use provenance_macros::rule;

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
    let tx = &mut transaction;
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
    let bound_values = database.max_bound_values();
    let (slot, reader) = (&mut report, &reader);
    database
        .write(Box::new(move |transaction| {
            Box::pin(async move {
                crate::test_probes::at("prepare_current_schema_under_guard")?;
                let digests = (manifest, global_digests);
                let (rebuilt, _) = rebuild_rows(transaction, reader, digests, bound_values).await?;
                *slot = Some(rebuilt);
                Ok(())
            })
        }))
        .await?;
    report.ok_or_else(|| crate::refusal::fault!("the read database ended a write without its work"))
}

/// Rebuilds in the caller's write transaction and commits it. Gives the
/// report and the committed revision. A statement binds at most
/// `bound_values` values.
pub(super) async fn rebuild_with_guard(
    guard: &publication::PublicationGuard,
    transaction: Lent,
    bound_values: usize,
) -> anyhow::Result<(MaterializeReport, (i64, String))> {
    let mut reader = validation::UnitReader::new(guard);
    let digests = reader.global(None)?;
    rebuild_rows(transaction, &reader, digests, bound_values).await
}

/// Hashes, validates, reloads, and commits one full rebuild in the
/// caller's write transaction. Gives the report and the committed
/// revision.
#[rule("rule_interrupted_migration_reloads_every_family")]
async fn rebuild_rows(
    mut transaction: Lent,
    reader: &validation::UnitReader<'_>,
    (manifest, global_digests): (provenance_core::Manifest, units::UnitDigests),
    bound_values: usize,
) -> anyhow::Result<(MaterializeReport, (i64, String))> {
    let tx = &mut transaction;
    let cache_recreated =
        current_schema::compatibility(tx).await? == Compatibility::RebuildRequired;
    if cache_recreated {
        current_schema::replace(tx).await?;
    } else {
        clear_cache(tx).await?;
    }
    let stored_serial = one_integer(
        &tx.fetch(
            "SELECT COALESCE(MAX(serial), 0) FROM projection_revision",
            Vec::new(),
        )
        .await?,
    )?;

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
            records_loaded +=
                family_rows::load_rows(tx, records.family, &records.bytes, bound_values).await?;
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
        relation_rows::load_rows(tx, &records.relations, scope, bound_values).await?;
        stamp::upsert_unit_row(tx, &unit.name(), &read.digests).await?;
        stamp::replace_file_rows(tx, &unit.name(), &read.files).await?;
    }
    stamp::write_stamp(tx, stored_serial + 1).await?;
    if cache_recreated {
        current_schema::mark_current(tx).await?;
    }
    crate::test_probes::at("materialize_before_commit")?;
    let rows = tx
        .fetch(
            "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
            Vec::new(),
        )
        .await?;
    let row = crate::read_database::one_row(&rows)?;
    let revision = (row.integer(0)?, row.text(1)?.to_owned());
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

fn one_integer(rows: &[crate::read_database::SqlRow]) -> anyhow::Result<i64> {
    crate::read_database::one_row(rows)?.integer(0)
}

async fn clear_cache(tx: &mut Lent) -> anyhow::Result<()> {
    for table in [
        "relations",
        "record_identities",
        "projection_family_digests",
        "projection_unit_digests",
        "projection_file_digests",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), Vec::new())
            .await?;
    }
    for family in super::ProjectionFamily::ALL {
        tx.execute(&format!("DELETE FROM {}", family.family_name()), Vec::new())
            .await?;
    }
    Ok(())
}
