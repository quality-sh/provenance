//! The pinned snapshot a read answers from, and the handles that read it.
//!
//! sqlx executes on `&mut` the connection, so every handle method takes
//! the transaction for the span of one statement and never across two
//! handles at once, which is how the operations read: one statement at a
//! time. The row readers behind the handles live in `cache::read`.

use crate::cache::quoted;
use crate::operations::stamp::{stored_revision, StoredRevision};
use provenance_core::model::ProjectionRow;
use provenance_core::ScopeId;
use provenance_macros::rule;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::{Mutex, PoisonError};

/// One read transaction at one revision.
///
/// A projection table is readable only through its handles, and each
/// handle records its family word, so the stamp's `attested` list names
/// every table the read opened.
#[rule("rule_stamp_attests_every_table_read")]
pub struct ReadSnapshot {
    tx: tokio::sync::Mutex<Transaction<'static, Sqlite>>,
    scope: ScopeId,
    revision: StoredRevision,
    attested: Mutex<BTreeSet<&'static str>>,
}

impl ReadSnapshot {
    /// Begins a read transaction on one pooled connection. Its first read
    /// is the stored revision, which pins the snapshot: every row read
    /// later in the transaction is at that serial. `None` means the
    /// database holds no revision.
    #[rule("rule_read_answers_from_one_pinned_transaction")]
    pub(crate) async fn open(pool: &SqlitePool, scope: &ScopeId) -> anyhow::Result<Option<Self>> {
        let mut tx = pool.begin().await?;
        let Some(revision) = stored_revision(&mut tx).await? else {
            return Ok(None);
        };
        Ok(Some(Self {
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

    fn attest(&self, word: &'static str) {
        self.attested
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(word);
    }

    /// The transaction, for the span of one statement.
    pub(crate) async fn connection(
        &self,
    ) -> tokio::sync::MutexGuard<'_, Transaction<'static, Sqlite>> {
        self.tx.lock().await
    }

    async fn count_rows(&self, table: &str) -> anyhow::Result<i64> {
        let mut tx = self.connection().await;
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM {} WHERE scope_id = ?",
            quoted(table)
        ))
        .bind(self.scope.as_str())
        .fetch_one(&mut **tx)
        .await?;
        Ok(count)
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

impl<'s> Relations<'s> {
    pub(crate) const fn snapshot(&self) -> &'s ReadSnapshot {
        self.snapshot
    }

    /// The scope's row count in the table.
    pub async fn count(&self) -> anyhow::Result<i64> {
        self.snapshot.count_rows("relations").await
    }
}
