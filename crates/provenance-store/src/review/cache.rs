use crate::state_store::StateStore;
use provenance_core::review::JournalEntry;
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

impl StateStore {
    pub(crate) fn validated_journal_entries(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<JournalEntry>> {
        self.validated_discussions(scope)?;
        self.validated_cycle_entries(scope)?;
        self.journal_entries(scope)
    }

    /// Reads the scope's decision-cycle receipts and refuses one that does not
    /// name records the scope holds.
    pub(crate) fn validated_cycle_entries(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<provenance_core::review::CycleEntry>> {
        super::decision_state::validated_cycle_entries(self, scope)
    }
}

pub async fn load_rows(tx: &mut Transaction<'_, Sqlite>, bytes: &[u8]) -> anyhow::Result<u64> {
    let entries: Vec<JournalEntry> = serde_json::from_slice(bytes)?;
    for JournalEntry::Cycle(entry) in &entries {
        sqlx::query("INSERT INTO review_journal(scope_id, kind, record_kind, record_id, id, sequence, request_id, payload) VALUES (?, 'cycle', ?, ?, ?, ?, ?, ?)")
            .bind(entry.scope_id.as_str()).bind(entry.record_kind.as_str()).bind(entry.record_id.as_str()).bind(entry.id.as_str())
            .bind(i64::try_from(entry.sequence)?).bind(entry.request_id.as_str()).bind(serde_json::to_string(entry)?)
            .execute(&mut **tx).await?;
    }
    Ok(entries.len() as u64)
}
