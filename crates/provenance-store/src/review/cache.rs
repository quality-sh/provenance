use crate::state_store::StateStore;
use provenance_core::review::{JournalEntry, ReviewEntry};
use provenance_core::ScopeId;
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, Transaction};
use std::io::{Read, Seek};

impl StateStore {
    pub(crate) fn validated_journal_entries(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<JournalEntry>> {
        let entries = self.validated_review_entries(scope)?;
        let discussion = self.discussion_entries(scope)?;
        self.discussion_heads(scope)?;
        for entry in &entries {
            if let Some(origin) = &entry.origin {
                self.validate_discussion_origin_among(scope, origin, &discussion)?;
            }
        }
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

    pub(crate) fn validated_review_entries(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ReviewEntry>> {
        let entries = self.review_entries(scope)?;
        let mut ids = Vec::new();
        for entry in &entries {
            ids.push((entry.record_kind, entry.record_id.as_str()));
        }
        ids.sort_by_key(|(kind, id)| (kind.rank(), *id));
        ids.dedup();
        for (kind, id) in ids {
            let chain = entries
                .iter()
                .filter(|e| e.record_kind == kind && e.record_id.as_str() == id)
                .cloned()
                .collect::<Vec<_>>();
            super::journal::validated_head(&chain)?;
        }
        let mut checked = std::collections::BTreeMap::new();
        for (reference, kind, owner) in entries.iter().flat_map(|e| {
            e.before
                .iter()
                .chain(std::iter::once(&e.after))
                .map(move |r| (r, e.record_kind, &e.record_id))
        }) {
            if let Some(previous) = checked.insert(reference.id.as_str(), (reference, kind, owner))
            {
                anyhow::ensure!(
                    previous == (reference, kind, owner),
                    "conflicting immutable snapshot references"
                );
                continue;
            }
            let path = super::journal::snapshot_path(&self.layout, scope, &reference.id);
            let mut file = super::journal::regular_file(&self.layout, &path)?;
            anyhow::ensure!(
                file.metadata()?.len() == reference.bytes,
                "review snapshot length differs from its immutable reference"
            );
            let mut hash = Sha256::new();
            let mut buffer = [0; 8192];
            loop {
                let size = file.read(&mut buffer)?;
                if size == 0 {
                    break;
                }
                hash.update(&buffer[..size]);
            }
            file.rewind()?;
            let snapshot: provenance_core::review::RecordSnapshot =
                serde_json::from_reader(&mut file)?;
            anyhow::ensure!(
                snapshot.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION
                    && snapshot.record.kind() == kind
                    && snapshot.record.scope_id() == scope
                    && snapshot.record.id() == owner
                    && reference.fields == super::snapshot::fields(&snapshot.record)?,
                "snapshot field index or address mismatch"
            );
            anyhow::ensure!(
                format!("sha256:{:x}", hash.finalize()) == reference.digest,
                "review snapshot digest differs from its immutable reference"
            );
        }
        Ok(entries)
    }
}

pub async fn load_rows(tx: &mut Transaction<'_, Sqlite>, bytes: &[u8]) -> anyhow::Result<u64> {
    let entries: Vec<JournalEntry> = serde_json::from_slice(bytes)?;
    for entry in &entries {
        match entry {
            JournalEntry::Record(entry) => {
                let row_kind = if entry.record_kind == provenance_core::NodeType::Requirement {
                    "requirement"
                } else {
                    "record"
                };
                sqlx::query("INSERT INTO review_journal(scope_id, kind, record_kind, record_id, id, sequence, request_id, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(entry.scope_id.as_str()).bind(row_kind).bind(entry.record_kind.as_str()).bind(entry.record_id.as_str()).bind(entry.id.as_str())
                    .bind(i64::try_from(entry.sequence)?).bind(entry.request_id.as_str()).bind(serde_json::to_string(entry)?)
                    .execute(&mut **tx).await?;
            }
            JournalEntry::Discussion(entry) => {
                sqlx::query("INSERT INTO review_journal(scope_id, kind, id, request_id, payload, discussion_id, thread_id, message_id, parent_type, parent_id, version) VALUES (?, 'discussion', ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                    .bind(entry.scope_id.as_str()).bind(entry.id.as_str()).bind(entry.request_id.as_str()).bind(serde_json::to_string(entry)?)
                    .bind(entry.discussion_id.as_str()).bind(entry.thread_id.as_str()).bind(entry.message_id.as_ref().map(provenance_core::StableId::as_str))
                    .bind(crate::state_store::serde_name(&entry.parent.node_type)?).bind(entry.parent.node_id.as_str()).bind(i64::try_from(entry.version)?)
                    .execute(&mut **tx).await?;
            }
            JournalEntry::Cycle(entry) => {
                sqlx::query("INSERT INTO review_journal(scope_id, kind, record_kind, record_id, id, sequence, request_id, payload) VALUES (?, 'cycle', ?, ?, ?, ?, ?, ?)")
                    .bind(entry.scope_id.as_str()).bind(entry.record_kind.as_str()).bind(entry.record_id.as_str()).bind(entry.id.as_str())
                    .bind(i64::try_from(entry.sequence)?).bind(entry.request_id.as_str()).bind(serde_json::to_string(entry)?)
                    .execute(&mut **tx).await?;
            }
        }
    }
    Ok(entries.len() as u64)
}
