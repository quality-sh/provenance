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
