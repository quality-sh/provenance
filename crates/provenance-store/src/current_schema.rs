//! The one SQLite schema for the disposable cache.

use crate::read_database::{SqlColumns, SqlValue, Transaction};
use sha2::{Digest, Sha256};

const DDL: &str = include_str!("../current_cache.sql");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Current,
    RebuildRequired,
}

/// Whether the schema of the database in `transaction` is the current
/// schema: its metadata row names the current digest and its catalog is
/// the current catalog.
pub async fn compatibility(transaction: &mut Transaction) -> anyhow::Result<Compatibility> {
    let table = transaction
        .fetch(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name = '_cache_metadata'",
            Vec::new(),
        )
        .await?;
    if table.is_empty() {
        return Ok(Compatibility::RebuildRequired);
    }
    let stored = match transaction
        .fetch(
            "SELECT schema_digest FROM _cache_metadata WHERE only_row = 1",
            Vec::new(),
        )
        .await
    {
        Ok(rows) => match rows.first() {
            Some(row) => row.optional_text(0)?.map(str::to_owned),
            None => None,
        },
        Err(error) if is_missing_column(&error) => return Ok(Compatibility::RebuildRequired),
        Err(error) => return Err(error),
    };
    let expected = expected_catalog();
    let live = catalog(transaction).await?;
    Ok(
        if stored.as_deref() == Some(expected.digest().as_str()) && &live == expected {
            Compatibility::Current
        } else {
            Compatibility::RebuildRequired
        },
    )
}

fn is_missing_column(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("no such column"))
}

/// Removes all cache tables and installs the current empty schema.
/// The caller records the digest only after it loads all rows and stamps.
pub async fn replace(transaction: &mut Transaction) -> anyhow::Result<()> {
    let tables = transaction
        .fetch(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            Vec::new(),
        )
        .await?;
    for table in tables {
        let sql = format!("DROP TABLE {}", quoted(table.text(0)?));
        transaction.execute(&sql, Vec::new()).await?;
    }
    for statement in statements() {
        transaction.execute(statement, Vec::new()).await?;
    }
    Ok(())
}

pub async fn mark_current(transaction: &mut Transaction) -> anyhow::Result<()> {
    let digest = expected_catalog().digest();
    transaction
        .execute(
            "INSERT INTO _cache_metadata (only_row, schema_digest) VALUES (1, ?)",
            vec![SqlValue::from(digest)],
        )
        .await?;
    Ok(())
}

/// The statements of the schema, in order.
fn statements() -> impl Iterator<Item = &'static str> {
    DDL.split(';').map(str::trim).filter(|sql| !sql.is_empty())
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
        format!("sha256:{}", hex::encode(hash.finalize()))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SchemaObject {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

/// The catalog that the schema gives. `SQLite` keeps the text of each
/// `CREATE` statement from its first keyword, so the catalog comes from
/// the schema text itself.
fn expected_catalog() -> &'static SchemaCatalog {
    static EXPECTED: std::sync::OnceLock<SchemaCatalog> = std::sync::OnceLock::new();
    EXPECTED.get_or_init(|| {
        let mut objects: Vec<SchemaObject> = statements().filter_map(schema_object).collect();
        objects.retain(|object| object.name != "_cache_metadata");
        objects.sort_by(|left, right| {
            (&left.kind, &left.name, &left.table).cmp(&(&right.kind, &right.name, &right.table))
        });
        SchemaCatalog(objects)
    })
}

/// The catalog row of one `CREATE TABLE` or `CREATE [UNIQUE] INDEX`
/// statement of the schema.
fn schema_object(statement: &str) -> Option<SchemaObject> {
    let create = statement.find("CREATE ")?;
    let sql = &statement[create..];
    let words: Vec<&str> = sql
        .split(|c: char| c.is_whitespace() || c == '(')
        .filter(|word| !word.is_empty())
        .collect();
    let (kind, name, table) = match words.as_slice() {
        ["CREATE", "TABLE", name, ..] => ("table", *name, *name),
        ["CREATE", "INDEX", name, "ON", table, ..]
        | ["CREATE", "UNIQUE", "INDEX", name, "ON", table, ..] => ("index", *name, *table),
        _ => return None,
    };
    Some(SchemaObject {
        kind: kind.to_owned(),
        name: name.to_owned(),
        table: table.to_owned(),
        sql: normalize_sql(sql),
    })
}

async fn catalog(transaction: &mut Transaction) -> anyhow::Result<SchemaCatalog> {
    let rows = transaction
        .fetch(
            "SELECT type, name, tbl_name, sql FROM sqlite_schema \
             WHERE name NOT LIKE 'sqlite_%' AND name <> '_cache_metadata' \
             ORDER BY type, name, tbl_name",
            Vec::new(),
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok(SchemaObject {
                kind: row.text(0)?.to_owned(),
                name: row.text(1)?.to_owned(),
                table: row.text(2)?.to_owned(),
                sql: normalize_sql(row.optional_text(3)?.unwrap_or_default()),
            })
        })
        .collect::<anyhow::Result<_>>()
        .map(SchemaCatalog)
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
#[cfg(feature = "host")]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Executor;

    #[test]
    fn schema_digest_is_stable_sha256_catalog() {
        let digest = expected_catalog().digest();
        assert_eq!(digest.len(), "sha256:".len() + 64);
        assert!(digest.starts_with("sha256:"));
    }

    /// The catalog from the schema text is the catalog that `SQLite` keeps
    /// after it runs the schema.
    #[tokio::test]
    async fn the_catalog_of_the_schema_text_is_the_catalog_sqlite_keeps() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for statement in statements() {
            pool.execute(statement).await.unwrap();
        }
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT type, name, tbl_name, sql FROM sqlite_schema \
             WHERE name NOT LIKE 'sqlite_%' AND name <> '_cache_metadata' \
             ORDER BY type, name, tbl_name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let kept = SchemaCatalog(
            rows.into_iter()
                .map(|(kind, name, table, sql)| SchemaObject {
                    kind,
                    name,
                    table,
                    sql: normalize_sql(sql.as_deref().unwrap_or_default()),
                })
                .collect(),
        );
        assert_eq!(&kept, expected_catalog());
        assert_eq!(expected_catalog().0.len(), 50);
        pool.close().await;
    }

    #[test]
    fn schema_sql_normalization_ignores_comments_and_whitespace() {
        let compact = "CREATE TABLE example(id TEXT NOT NULL)";
        let formatted = "/* comment */ CREATE TABLE example ( id TEXT -- comment\n NOT NULL )";
        assert_eq!(normalize_sql(compact), normalize_sql(formatted));
    }
}
