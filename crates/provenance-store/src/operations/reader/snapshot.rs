//! The pinned snapshot a read answers from, and the handles that read it.
//!
//! sqlx executes on `&mut` the connection, so every handle method takes
//! the transaction for the span of one statement and never across two
//! handles at once, which is how the operations read: one statement at a
//! time.

use crate::cache::ProjectionFamily;
use crate::operations::stamp::{stored_revision, StoredRevision};
use provenance_core::ScopeId;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

/// One read transaction at one revision.
pub struct ReadSnapshot {
    tx: tokio::sync::Mutex<Transaction<'static, Sqlite>>,
    scope: ScopeId,
    revision: StoredRevision,
    attested: Mutex<BTreeSet<&'static str>>,
}

impl ReadSnapshot {
    /// Begins a read transaction on one pooled connection. Its first read
    /// is the stored revision, which pins the snapshot: every row read
    /// later in the transaction is at that serial by the rule of `SQLite` itself.
    /// `None` means the database holds no revision.
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

    /// A handle on one kind or integration table; taking it puts the
    /// family word in `attested`.
    pub fn table(&self, family: ProjectionFamily) -> Table<'_> {
        self.attest(family.family_name());
        Table {
            snapshot: self,
            family,
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

    async fn count_rows(&self, table: &str) -> anyhow::Result<i64> {
        let mut tx = self.tx.lock().await;
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE scope_id = ?"))
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

/// One projection table at the snapshot's revision.
pub struct Table<'s> {
    snapshot: &'s ReadSnapshot,
    family: ProjectionFamily,
}

impl Table<'_> {
    pub const fn family(&self) -> ProjectionFamily {
        self.family
    }

    /// The scope's row count in the table.
    pub async fn count(&self) -> anyhow::Result<i64> {
        self.snapshot.count_rows(self.family.family_name()).await
    }
}

/// The derived `relations` table at the snapshot's revision.
pub struct Relations<'s> {
    snapshot: &'s ReadSnapshot,
}

impl Relations<'_> {
    /// The scope's row count in the table.
    pub async fn count(&self) -> anyhow::Result<i64> {
        self.snapshot.count_rows("relations").await
    }
}
