use crate::operations::reader::ReadSnapshot;
use crate::read_database::{SqlColumns, SqlRow};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::{Message, ScopeId, StableId, Thread, ThreadParent};
use serde::de::DeserializeOwned;

/// The columns of a thread, in the order that `thread` reads them.
const THREAD_COLUMNS: [&str; 6] = [
    "scope_id",
    "id",
    "parent_type",
    "parent_id",
    "status",
    "created_at",
];

/// The columns of a message, in the order that `message` reads them.
const MESSAGE_COLUMNS: [&str; 6] = ["scope_id", "id", "thread_id", "role", "body", "created_at"];

fn word<T: DeserializeOwned>(text: &str) -> anyhow::Result<T> {
    Ok(serde_json::from_value(serde_json::Value::String(
        text.to_owned(),
    ))?)
}

fn thread(mut row: SqlRow) -> anyhow::Result<Thread> {
    Ok(Thread {
        scope_id: ScopeId::new(row.text(0)?)?,
        id: StableId::new(row.take_text(1)?)?,
        parent: ThreadParent {
            node_type: word(row.text(2)?)?,
            node_id: StableId::new(row.take_text(3)?)?,
        },
        status: word(row.text(4)?)?,
        created_at: row.integer(5)?,
    })
}

fn message(mut row: SqlRow) -> anyhow::Result<Message> {
    Ok(Message {
        scope_id: ScopeId::new(row.text(0)?)?,
        id: StableId::new(row.take_text(1)?)?,
        thread_id: StableId::new(row.take_text(2)?)?,
        role: word(row.text(3)?)?,
        body: row.take_text(4)?,
        created_at: row.integer(5)?,
    })
}

impl ReadSnapshot {
    /// All discussions at the same revision as the graph records.
    pub async fn threads(&self) -> anyhow::Result<Vec<Thread>> {
        self.attest("threads");
        let sql = format!(
            "SELECT {} FROM threads WHERE scope_id = ? ORDER BY created_at, id",
            THREAD_COLUMNS.join(", ")
        );
        let rows = self.fetch(&sql, vec![self.scope_value()]).await?;
        rows.into_iter().map(thread).collect()
    }

    /// All messages at the graph revision, in logical order with ID ties.
    pub async fn messages(&self) -> anyhow::Result<Vec<Message>> {
        self.attest("messages");
        let sql = format!(
            "SELECT {} FROM messages WHERE scope_id = ? ORDER BY created_at, id",
            MESSAGE_COLUMNS.join(", ")
        );
        let rows = self.fetch(&sql, vec![self.scope_value()]).await?;
        rows.into_iter().map(message).collect()
    }

    pub(crate) async fn page_thread(&self, id: &str) -> anyhow::Result<Option<Thread>> {
        self.attest("threads");
        let rows = self.page_row("threads", &THREAD_COLUMNS, id).await?;
        rows.into_iter()
            .map(thread)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|mut rows| rows.pop())
    }

    pub(crate) async fn page_message(&self, id: &str) -> anyhow::Result<Option<Message>> {
        self.attest("messages");
        let rows = self.page_row("messages", &MESSAGE_COLUMNS, id).await?;
        rows.into_iter()
            .map(message)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|mut rows| rows.pop())
    }

    /// The row of `table` with the id, after its stored byte count over
    /// `columns` passes the record ceiling.
    async fn page_row(
        &self,
        table: &str,
        columns: &[&str],
        id: &str,
    ) -> anyhow::Result<Vec<crate::read_database::SqlRow>> {
        let values = vec![self.scope_value(), id.into()];
        let sizes = self
            .fetch(
                &format!(
                    "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
                    super::page::byte_expression(columns),
                    crate::cache::quoted(table)
                ),
                values.clone(),
            )
            .await?;
        let maximum = i64::try_from(super::page::RECORD_BYTES)?;
        if let Some(size) = sizes.first() {
            if size.integer(0)? > maximum {
                return Err(ReadFailure::PageRecordTooLarge.into());
            }
        }
        let sql = format!(
            "SELECT {} FROM {table} WHERE scope_id = ? AND id = ? LIMIT 1",
            columns.join(", ")
        );
        self.fetch(&sql, values).await
    }
}
