//! The one SQLite schema for the disposable cache.

use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Executor, Sqlite, SqlitePool, Transaction};

const DDL: &str = include_str!("../current_cache.sql");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Current,
    RebuildRequired,
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
    let expected = expected_catalog().await?;
    let live = catalog(pool).await?;
    Ok(if stored.as_deref() == Some(expected.digest().as_str()) && live == expected {
        Compatibility::Current
    } else {
        Compatibility::RebuildRequired
    })
}

/// Removes all cache tables and installs the current empty schema.
/// The caller records the digest only after it loads all rows and stamps.
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
    let digest = expected_catalog().await?.digest();
    sqlx::query("INSERT INTO _cache_metadata (only_row, schema_digest) VALUES (1, ?)")
        .bind(digest)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct SchemaCatalog(Vec<SchemaObject>);

impl SchemaCatalog {
    fn digest(&self) -> String {
        let mut hash = Sha256::new();
        for object in &self.0 {
            for value in [&object.kind, &object.name, &object.table, &object.sql] {
                hash.update(value.as_bytes());
                hash.update([0]);
            }
        }
        format!("sha256:{hash:x}")
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SchemaObject {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

async fn expected_catalog() -> anyhow::Result<SchemaCatalog> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    for statement in DDL.split(';').map(str::trim).filter(|sql| !sql.is_empty()) {
        pool.execute(statement).await?;
    }
    let expected = catalog(&pool).await;
    pool.close().await;
    expected
}

async fn catalog(pool: &SqlitePool) -> anyhow::Result<SchemaCatalog> {
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT type, name, tbl_name, sql FROM sqlite_schema \
         WHERE name NOT LIKE 'sqlite_%' AND name <> '_cache_metadata' \
         ORDER BY type, name, tbl_name",
    )
    .fetch_all(pool)
    .await?;
    Ok(SchemaCatalog(
        rows.into_iter()
            .map(|(kind, name, table, sql)| SchemaObject {
                kind,
                name,
                table,
                sql: normalize_sql(sql.as_deref().unwrap_or_default()),
            })
            .collect(),
    ))
}

fn normalize_sql(sql: &str) -> String {
    let mut normalized = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut quote = None;
    while let Some(character) = chars.next() {
        if let Some(end) = quote {
            normalized.push(character);
            if character == end {
                if chars.peek() == Some(&end) {
                    normalized.push(chars.next().expect("peeked quote"));
                } else {
                    quote = None;
                }
            }
            continue;
        }
        match character {
            '\'' | '"' | '`' => {
                quote = Some(character);
                normalized.push(character);
            }
            '[' => {
                quote = Some(']');
                normalized.push(character);
            }
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
            }
            character if character.is_whitespace() => {}
            character => normalized.push(character),
        }
    }
    normalized
}

fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn schema_digest_is_stable_sha256_catalog() {
        let digest = expected_catalog().await.unwrap().digest();
        assert_eq!(digest.len(), "sha256:".len() + 64);
        assert!(digest.starts_with("sha256:"));
    }

    #[test]
    fn schema_sql_normalization_ignores_comments_and_whitespace() {
        let compact = "CREATE TABLE example(id TEXT NOT NULL)";
        let formatted = "/* comment */ CREATE TABLE example ( id TEXT -- comment\n NOT NULL )";
        assert_eq!(normalize_sql(compact), normalize_sql(formatted));
    }
}
