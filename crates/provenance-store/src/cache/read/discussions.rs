use crate::operations::reader::ReadSnapshot;
use provenance_core::{Message, ScopeId, StableId, Thread, ThreadParent, SUPPORTED_SCHEMA_VERSION};
use serde::de::DeserializeOwned;
use sqlx::Row;

fn word<T: DeserializeOwned>(text: String) -> anyhow::Result<T> {
    Ok(serde_json::from_value(serde_json::Value::String(text))?)
}

impl ReadSnapshot {
    /// All discussions at the same revision as the graph records.
    pub async fn threads(&self) -> anyhow::Result<Vec<Thread>> {
        self.attest("threads");
        let mut tx = self.connection().await;
        let rows = sqlx::query("SELECT * FROM threads WHERE scope_id = ? ORDER BY created_at, id")
            .bind(self.scope().as_str())
            .fetch_all(&mut **tx)
            .await?;
        drop(tx);
        rows.iter()
            .map(|row| {
                Ok(Thread {
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: ScopeId::new(row.try_get::<String, _>("scope_id")?)?,
                    id: StableId::new(row.try_get::<String, _>("id")?)?,
                    parent: ThreadParent {
                        node_type: word(row.try_get("parent_type")?)?,
                        node_id: StableId::new(row.try_get::<String, _>("parent_id")?)?,
                    },
                    status: word(row.try_get("status")?)?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    /// All messages at the graph revision, in logical order with ID ties.
    pub async fn messages(&self) -> anyhow::Result<Vec<Message>> {
        self.attest("messages");
        let mut tx = self.connection().await;
        let rows = sqlx::query("SELECT * FROM messages WHERE scope_id = ? ORDER BY created_at, id")
            .bind(self.scope().as_str())
            .fetch_all(&mut **tx)
            .await?;
        drop(tx);
        rows.iter()
            .map(|row| {
                Ok(Message {
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: ScopeId::new(row.try_get::<String, _>("scope_id")?)?,
                    id: StableId::new(row.try_get::<String, _>("id")?)?,
                    thread_id: StableId::new(row.try_get::<String, _>("thread_id")?)?,
                    role: word(row.try_get("role")?)?,
                    body: row.try_get("body")?,
                    created_at: row.try_get("created_at")?,
                    ai_metadata: row
                        .try_get::<Option<String>, _>("ai_metadata")?
                        .map(|value| serde_json::from_str(&value))
                        .transpose()?,
                })
            })
            .collect()
    }
}

impl ReadSnapshot {
    pub(crate) async fn page_thread(&self, id: &str) -> anyhow::Result<Option<Thread>> {
        self.attest("threads");
        let mut tx = self.connection().await;
        check_size(
            &mut tx,
            self.scope().as_str(),
            "threads",
            id,
            &[
                "scope_id",
                "id",
                "parent_type",
                "parent_id",
                "status",
                "created_at",
            ],
        )
        .await?;
        let rows = sqlx::query("SELECT * FROM threads WHERE scope_id = ? AND id = ? LIMIT 1")
            .bind(self.scope().as_str())
            .bind(id)
            .fetch_all(&mut **tx)
            .await?;
        drop(tx);
        rows.iter()
            .map(|row| {
                Ok(Thread {
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: ScopeId::new(row.try_get::<String, _>("scope_id")?)?,
                    id: StableId::new(row.try_get::<String, _>("id")?)?,
                    parent: ThreadParent {
                        node_type: word(row.try_get("parent_type")?)?,
                        node_id: StableId::new(row.try_get::<String, _>("parent_id")?)?,
                    },
                    status: word(row.try_get("status")?)?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|mut rows| rows.pop())
    }

    pub(crate) async fn page_message(&self, id: &str) -> anyhow::Result<Option<Message>> {
        self.attest("messages");
        let mut tx = self.connection().await;
        check_size(
            &mut tx,
            self.scope().as_str(),
            "messages",
            id,
            &[
                "scope_id",
                "id",
                "thread_id",
                "role",
                "body",
                "created_at",
                "ai_metadata",
            ],
        )
        .await?;
        let rows = sqlx::query("SELECT * FROM messages WHERE scope_id = ? AND id = ? LIMIT 1")
            .bind(self.scope().as_str())
            .bind(id)
            .fetch_all(&mut **tx)
            .await?;
        drop(tx);
        rows.iter()
            .map(|row| {
                Ok(Message {
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: ScopeId::new(row.try_get::<String, _>("scope_id")?)?,
                    id: StableId::new(row.try_get::<String, _>("id")?)?,
                    thread_id: StableId::new(row.try_get::<String, _>("thread_id")?)?,
                    role: word(row.try_get("role")?)?,
                    body: row.try_get("body")?,
                    created_at: row.try_get("created_at")?,
                    ai_metadata: row
                        .try_get::<Option<String>, _>("ai_metadata")?
                        .map(|value| serde_json::from_str(&value))
                        .transpose()?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|mut rows| rows.pop())
    }
}

async fn check_size(
    tx: &mut sqlx::SqliteConnection,
    scope: &str,
    table: &str,
    id: &str,
    columns: &[&str],
) -> anyhow::Result<()> {
    let size: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
        super::page::byte_expression(columns),
        crate::cache::quoted(table)
    ))
    .bind(scope)
    .bind(id)
    .fetch_optional(tx)
    .await?;
    let maximum = i64::try_from(super::page::RECORD_BYTES)?;
    if size.is_some_and(|size| size > maximum) {
        return Err(
            provenance_core::protocol::read_failure::ReadFailure::PageRecordTooLarge.into(),
        );
    }
    Ok(())
}
