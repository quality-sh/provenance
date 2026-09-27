//! The one SQLite schema for the disposable cache.

use provenance_macros::rule;
use sha2::{Digest, Sha256};
use sqlx::{Executor, Sqlite, SqlitePool, Transaction};

const DDL: &str = include_str!("../current_cache.sql");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Current,
    RebuildRequired,
}

pub fn digest() -> String {
    format!("sha256:{:x}", Sha256::digest(DDL.as_bytes()))
}

pub async fn compatibility(pool: &SqlitePool) -> anyhow::Result<Compatibility> {
    let table: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name = '_cache_metadata'",
    )
    .fetch_optional(pool)
    .await?;
    if table.is_none() {
        return Ok(Compatibility::RebuildRequired);
    }
    let stored: Option<String> =
        match sqlx::query_scalar("SELECT schema_digest FROM _cache_metadata WHERE only_row = 1")
            .fetch_optional(pool)
            .await
        {
            Ok(stored) => stored,
            Err(sqlx::Error::Database(error)) if error.message().contains("no such column") => {
                return Ok(Compatibility::RebuildRequired);
            }
            Err(error) => return Err(error.into()),
        };
    Ok(if stored.as_deref() == Some(digest().as_str()) {
        Compatibility::Current
    } else {
        Compatibility::RebuildRequired
    })
}

/// Removes all cache tables and installs the current empty schema.
/// The caller records the digest only after it loads all rows and stamps.
#[rule("rule_interrupted_migration_reloads_every_family")]
pub async fn replace(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&mut **tx)
    .await?;
    for table in tables {
        tx.execute(format!("DROP TABLE {}", quoted(&table)).as_str())
            .await?;
    }
    for statement in DDL.split(';').map(str::trim).filter(|sql| !sql.is_empty()) {
        tx.execute(statement).await?;
    }
    Ok(())
}

pub async fn mark_current(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO _cache_metadata (only_row, schema_digest) VALUES (1, ?)")
        .bind(digest())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_digest_is_stable_sha256_text() {
        let digest = digest();
        assert_eq!(digest.len(), "sha256:".len() + 64);
        assert!(digest.starts_with("sha256:"));
    }
}
