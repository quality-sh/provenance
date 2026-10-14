//! Current document reconciliation, using the canonical bounded document read.
use super::output::{Continuation, Continuity, Document, OutputFrame};
use provenance_cli::repo_context::RepoContext;
use provenance_core::protocol::ReadDocumentQuery;
use provenance_macros::rule;
use provenance_store::operations::{queries, read_policy::ReadPolicy};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Default)]
pub(in crate::review) struct Baseline {
    records: BTreeMap<String, Value>,
}

impl Baseline {
    /// Returns bounded current state after a notification; no notification history is stored.
    #[rule("rule_review_follow_reset")]
    pub async fn reconcile(
        &mut self,
        context: &RepoContext,
        root: &str,
        limit: usize,
        epoch: &str,
        sequence: u64,
        reset: bool,
    ) -> anyhow::Result<Vec<OutputFrame>> {
        let mut cursor = None;
        let mut current = BTreeMap::new();
        let mut pages = Vec::new();
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
            let continuations = cursor
                .as_ref()
                .map(|cursor| {
                    vec![Continuation {
                        root_id: root.to_owned(),
                        cursor: cursor.clone(),
                    }]
                })
                .unwrap_or_default();
            pages.push((records, continuations));
            if cursor.is_none() {
                break;
            }
        }
        let removed = self
            .records
            .keys()
            .filter(|id| !current.contains_key(*id))
            .cloned()
            .collect::<Vec<_>>();
        let mut frames = Vec::new();
        if reset || self.records != current {
            let last = pages.len().saturating_sub(1);
            for (index, (records, continuations)) in pages.into_iter().enumerate() {
                let document = Document {
                    epoch: epoch.to_owned(),
                    sequence,
                    continuity: if reset || self.records.is_empty() {
                        Continuity::Unknown
                    } else {
                        Continuity::Current
                    },
                    records,
                    removed_ids: if index == last {
                        removed.clone()
                    } else {
                        vec![]
                    },
                    continuations,
                };
                frames.push(if reset {
                    OutputFrame::Reset { document }
                } else {
                    OutputFrame::State { document }
                });
            }
        }
        self.records = current;
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
