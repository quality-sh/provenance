use crate::cache::revision_digest_from_stored_rows;
use crate::read_database::{SqlColumns, SqlValue, Transaction};

/// Commits the revision and validation version with the parsed rows and their digests.
pub(super) async fn write_stamp(tx: &mut Transaction, serial: i64) -> anyhow::Result<()> {
    let rows = tx
        .fetch(
            "SELECT scope_id, family, content_digest, record_count FROM projection_family_digests",
            Vec::new(),
        )
        .await?
        .iter()
        .map(|row| {
            Ok((
                row.text(0)?.to_owned(),
                row.text(1)?.to_owned(),
                row.text(2)?.to_owned(),
                row.integer(3)?,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let revision = revision_digest_from_stored_rows(&rows)?;
    tx.execute(
        "INSERT OR IGNORE INTO projection_instance (only_row, instance_id) VALUES (1, ?)",
        vec![SqlValue::from(uuid::Uuid::new_v4().to_string())],
    )
    .await?;
    insert_revision(tx, serial, &revision).await?;
    tx.execute(
        "INSERT OR REPLACE INTO projection_validation (only_row, version) VALUES (1, ?)",
        vec![SqlValue::Integer(i64::from(crate::VALIDATION_VERSION))],
    )
    .await?;
    crate::test_probes::at("stamp_before_unit_digests")?;
    Ok(())
}

pub(super) async fn insert_revision(
    tx: &mut Transaction,
    serial: i64,
    digest: &str,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO projection_revision (serial, digest) VALUES (?, ?)",
        vec![SqlValue::Integer(serial), SqlValue::from(digest)],
    )
    .await?;
    Ok(())
}

pub(super) async fn upsert_content_row(
    tx: &mut Transaction,
    scope_id: &str,
    family: &str,
    content_digest: &str,
    stored_digest: &str,
    record_count: i64,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO projection_family_digests \
         (scope_id, family, content_digest, stored_digest, record_count) VALUES (?, ?, ?, ?, ?)",
        vec![
            SqlValue::from(scope_id),
            SqlValue::from(family),
            SqlValue::from(content_digest),
            SqlValue::from(stored_digest),
            SqlValue::Integer(record_count),
        ],
    )
    .await?;
    Ok(())
}

pub(super) async fn upsert_unit_row(
    tx: &mut Transaction,
    unit: &str,
    digests: &super::units::UnitDigests,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO projection_unit_digests (unit, digest, stored_digest) \
         VALUES (?, ?, ?)",
        vec![
            SqlValue::from(unit),
            SqlValue::from(digests.content.as_str()),
            SqlValue::from(digests.stored.as_str()),
        ],
    )
    .await?;
    Ok(())
}

/// Replaces the file rows of one unit with `files`.
pub(super) async fn replace_file_rows(
    tx: &mut Transaction,
    unit: &str,
    files: &std::collections::BTreeMap<String, super::units::FileDigest>,
) -> anyhow::Result<()> {
    tx.execute(
        "DELETE FROM projection_file_digests WHERE unit = ?",
        vec![SqlValue::from(unit)],
    )
    .await?;
    for (path, file) in files {
        tx.execute(
            "INSERT INTO projection_file_digests (unit, path, stored_digest, content_digest) \
             VALUES (?, ?, ?, ?)",
            vec![
                SqlValue::from(unit),
                SqlValue::from(path.as_str()),
                SqlValue::from(file.stored.as_str()),
                SqlValue::from(file.content.as_str()),
            ],
        )
        .await?;
    }
    Ok(())
}
