//! The pinned snapshot a read answers from, and the handles that read it.
//!
//! A transaction runs one statement at a time, so every handle method
//! takes the transaction for the span of one statement and never across
//! two handles at once, which is how the operations read: one statement at
//! a time. The row readers behind the handles live in `cache::read`.

use crate::cache::quoted;
use crate::operations::stamp::{stored_revision, StoredRevision};
use crate::read_database::file::FileSqlx as _;
use crate::read_database::{SqlColumns, SqlValue, Transaction};
use provenance_core::model::ProjectionRow;
use provenance_core::ScopeId;
use provenance_macros::rule;
use sqlx::Sqlite;
use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::{Mutex, PoisonError};
use tokio::sync::{MappedMutexGuard, MutexGuard};

/// One read transaction at one revision.
///
/// A projection table is readable only through its handles, and each
/// handle records its family word, so the stamp's `attested` list names
/// every table the read opened.
#[rule("rule_stamp_attests_every_table_read")]
pub struct ReadSnapshot {
    tx: tokio::sync::Mutex<Transaction>,
    /// The most values of one list that a statement of the read binds.
    list_values: usize,
    scope: ScopeId,
    revision: StoredRevision,
    attested: Mutex<BTreeSet<&'static str>>,
}

impl ReadSnapshot {
    /// Takes one read transaction of the read database. Its first read
    /// is the stored revision, which pins the snapshot: every row read
    /// later in the transaction is at that serial. `None` means the
    /// database holds no revision.
    #[rule("rule_read_answers_from_one_pinned_transaction")]
    pub(crate) async fn open(mut tx: Transaction, scope: &ScopeId) -> anyhow::Result<Option<Self>> {
        let Some(revision) = stored_revision(tx.as_mut()).await? else {
            return Ok(None);
        };
        Ok(Some(Self {
            list_values: tx.max_list_values(),
            tx: tokio::sync::Mutex::new(tx),
            scope: scope.clone(),
            revision,
            attested: Mutex::new(BTreeSet::new()),
        }))
    }

    pub const fn serial(&self) -> i64 {
        self.revision.serial
    }

    pub fn digest(&self) -> &str {
        &self.revision.digest
    }

    pub fn instance_id(&self) -> &str {
        &self.revision.instance_id
    }

    pub(crate) const fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// A handle on one kind or integration table; taking it puts the
    /// table's family word in `attested`.
    pub fn table<K: ProjectionRow>(&self) -> Table<'_, K> {
        self.attest(K::TABLE);
        Table {
            snapshot: self,
            kind: PhantomData,
        }
    }

    /// A handle on the derived `relations` table; taking it puts
    /// `relations` in `attested`.
    pub fn relations(&self) -> Relations<'_> {
        self.attest("relations");
        Relations { snapshot: self }
    }

    pub(crate) fn attest(&self, word: &'static str) {
        self.attested
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(word);
    }

    /// The transaction, for the span of one statement.
    pub(crate) async fn transaction(&self) -> MutexGuard<'_, Transaction> {
        self.tx.lock().await
    }

    /// Runs one statement of the read and gives its rows.
    pub(crate) async fn fetch(
        &self,
        sql: &str,
        values: Vec<SqlValue>,
    ) -> anyhow::Result<Vec<crate::read_database::SqlRow>> {
        self.transaction().await.fetch(sql, values).await
    }

    /// The most values of one list that a statement of the read binds; a
    /// reader puts a longer list in more statements.
    pub(crate) const fn list_values(&self) -> usize {
        self.list_values
    }

    /// The scope as the value that a statement binds.
    pub(crate) fn scope_value(&self) -> SqlValue {
        SqlValue::from(self.scope.as_str())
    }

    /// The `SQLx` transaction of the file adapter, for the span of one
    /// statement of a reader that still runs `SQLx` directly. A read of
    /// another adapter refuses here with a fault.
    pub(crate) async fn connection(&self) -> anyhow::Result<FileConnection<'_>> {
        MutexGuard::try_map(self.tx.lock().await, |tx| tx.sqlx())
            .map_err(|_| crate::refusal::fault!("{NOT_A_FILE}"))
    }

    async fn count_rows(&self, table: &str) -> anyhow::Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {} WHERE scope_id = ?", quoted(table));
        let rows = self
            .transaction()
            .await
            .fetch(&sql, vec![self.scope_value()])
            .await?;
        one_row(&rows)?.integer(0)
    }

    /// Ends the read and hands back what the stamp needs. The transaction
    /// rolls back on drop; it wrote nothing.
    pub(crate) fn finish(self) -> (StoredRevision, BTreeSet<&'static str>) {
        let attested = self
            .attested
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        (self.revision, attested)
    }
}

/// The transaction of a snapshot as the `SQLx` transaction of the file
/// adapter. Only the readers that run `SQLx` take it.
pub type FileConnection<'s> = MappedMutexGuard<'s, sqlx::Transaction<'static, Sqlite>>;

/// The fault of a read of another adapter that reaches a reader that runs
/// `SQLx`. It ends when the discussion reads run on the port.
const NOT_A_FILE: &str =
    "the discussion reads read only the read database file under .provenance/cache";

/// One record table at the snapshot's revision, typed by the record it
/// holds. The marker is a function type, so the handle is `Send` and
/// `Sync` whatever the record type is.
pub struct Table<'s, K> {
    snapshot: &'s ReadSnapshot,
    kind: PhantomData<fn() -> K>,
}

impl<'s, K: ProjectionRow> Table<'s, K> {
    pub(crate) const fn snapshot(&self) -> &'s ReadSnapshot {
        self.snapshot
    }

    /// The scope's row count in the table.
    pub async fn count(&self) -> anyhow::Result<i64> {
        self.snapshot.count_rows(K::TABLE).await
    }
}

/// The derived `relations` table at the snapshot's revision.
pub struct Relations<'s> {
    snapshot: &'s ReadSnapshot,
}

/// One repository identity row before scope and kind authorization.
pub struct RecordIdentity {
    pub scope_id: ScopeId,
    pub node_type: provenance_core::NodeType,
}

/// Repository identity rows in this snapshot.
pub struct RecordIdentities<'s> {
    snapshot: &'s ReadSnapshot,
}

impl<'s> Relations<'s> {
    pub(crate) const fn snapshot(&self) -> &'s ReadSnapshot {
        self.snapshot
    }

    /// The scope's row count in the table.
    pub async fn count(&self) -> anyhow::Result<i64> {
        self.snapshot.count_rows("relations").await
    }
}

impl ReadSnapshot {
    pub fn record_identities(&self) -> RecordIdentities<'_> {
        self.attest("record_identities");
        RecordIdentities { snapshot: self }
    }
}

impl RecordIdentities<'_> {
    pub async fn resolve(&self, id: &str) -> anyhow::Result<Vec<RecordIdentity>> {
        let rows = self
            .snapshot
            .transaction()
            .await
            .fetch(
                "SELECT scope_id, node_type FROM record_identities \
                 WHERE id = ? ORDER BY scope_id, node_type",
                vec![SqlValue::from(id)],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(RecordIdentity {
                    scope_id: ScopeId::new(row.text(0)?)?,
                    node_type: provenance_core::NodeType::parse(row.text(1)?)?,
                })
            })
            .collect()
    }
}

/// The one row of a statement that gives exactly one, as an aggregate does.
pub fn one_row(rows: &[crate::read_database::SqlRow]) -> anyhow::Result<&[SqlValue]> {
    rows.first().map(Vec::as_slice).ok_or_else(|| {
        crate::refusal::fault!(
            "no rows returned by a query that expected to return at least one row"
        )
    })
}
