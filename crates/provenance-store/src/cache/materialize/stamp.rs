use crate::cache::revision_digest_from_stored_rows;
use sqlx::{Sqlite, Transaction};

/// Commits the revision and validation version with the parsed rows and their digests.
pub(super) async fn write_stamp(
    tx: &mut Transaction<'_, Sqlite>,
    serial: i64,
) -> anyhow::Result<()> {
    let rows = sqlx::query_as(
        "SELECT scope_id, family, content_digest, record_count FROM projection_family_digests",
    )
    .fetch_all(&mut **tx)
    .await?;
    let revision = revision_digest_from_stored_rows(&rows)?;
    sqlx::query("INSERT OR IGNORE INTO projection_instance (only_row, instance_id) VALUES (1, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&mut **tx)
        .await?;
    insert_revision(tx, serial, &revision).await?;
    sqlx::query("INSERT OR REPLACE INTO projection_validation (only_row, version) VALUES (1, ?)")
        .bind(i64::from(crate::VALIDATION_VERSION))
        .execute(&mut **tx)
        .await?;
    crate::test_probes::at("stamp_before_unit_digests")?;
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
    digests: &super::units::UnitDigests,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO projection_unit_digests (unit, digest, stored_digest) \
         VALUES (?, ?, ?)",
    )
    .bind(unit)
    .bind(&digests.content)
    .bind(&digests.stored)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
