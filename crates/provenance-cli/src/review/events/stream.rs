//! Current document reconciliation, using the canonical bounded document read.
use super::output::{Continuity, Document, OutputFrame};
use provenance_cli::repo_context::RepoContext;
use provenance_core::protocol::ReadDocumentQuery;
use provenance_macros::rule;
use provenance_store::operations::{queries, read_policy::ReadPolicy};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(in crate::review) struct Baseline {
    records: BTreeMap<String, Value>,
    initialized: bool,
}

impl Baseline {
    /// Compares one complete document with the current baseline; no journal is stored.
    #[rule("rule_review_follow_reset")]
    pub async fn reconcile(
        &mut self,
        context: &RepoContext,
        root: &str,
        limit: usize,
        position: (&str, u64),
        reset: bool,
        selected: Option<&BTreeSet<String>>,
    ) -> anyhow::Result<Vec<OutputFrame>> {
        let (epoch, sequence) = position;
        let mut cursor = None;
        let mut current = BTreeMap::new();
        let mut ordered = Vec::new();
        loop {
            let page = queries::read_document(
                Some(context.repo.clone()),
                &context.scope_id()?,
                ReadPolicy::default(),
                ReadDocumentQuery {
                    id: root.to_owned(),
                    exclude_terminal: false,
                    cursor,
                    limit,
                },
            )
            .await?
            .result;
            let records = page
                .entries
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?;
            for record in &records {
                let id = record_id(record)?;
                current.insert(id, record.clone());
            }
            cursor = page.next_cursor;
            ordered.extend(records);
            if cursor.is_none() || limit == 1 {
                break;
            }
        }
        let full = reset || !self.initialized;
        let removed = self
            .records
            .keys()
            .filter(|id| {
                !current.contains_key(*id) && (full || selected.is_none_or(|ids| ids.contains(*id)))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut frames = Vec::new();
        let records = ordered
            .into_iter()
            .filter(|record| {
                full || record_id(record).is_ok_and(|id| {
                    selected.is_none_or(|ids| ids.contains(&id))
                        && self.records.get(&id) != Some(record)
                })
            })
            .collect::<Vec<_>>();
        if full || !records.is_empty() || !removed.is_empty() {
            let document = Document {
                epoch: epoch.to_owned(),
                sequence,
                continuity: if full {
                    Continuity::Unknown
                } else {
                    Continuity::Current
                },
                records,
                removed_ids: removed,
                continuations: vec![],
            };
            frames.push(if reset {
                OutputFrame::Reset { document }
            } else {
                OutputFrame::State { document }
            });
        }
        self.initialized = true;
        if full || selected.is_none() {
            self.records = current;
        } else if let Some(ids) = selected {
            // A later page notice must still compare its IDs with the delivered baseline.
            for id in ids {
                if let Some(record) = current.get(id) {
                    self.records.insert(id.clone(), record.clone());
                } else {
                    self.records.remove(id);
                }
            }
        }
        Ok(frames)
    }
}

fn record_id(entry: &Value) -> anyhow::Result<String> {
    ["node", "thread", "message"]
        .iter()
        .find_map(|key| entry[*key]["id"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("document entry has no record identity"))
}
