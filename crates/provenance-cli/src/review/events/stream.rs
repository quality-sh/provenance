//! Current document reconciliation, using the canonical bounded document read.
use provenance_cli::repo_context::RepoContext;
use provenance_core::protocol::ReadDocumentQuery;
use provenance_macros::rule;
use provenance_store::operations::{queries, read_policy::ReadPolicy};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Write};

#[derive(Default)]
pub(crate) struct Baseline {
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
    ) -> anyhow::Result<()> {
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
                .map(|cursor| vec![json!({"rootId": root, "cursor": cursor})])
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
        if reset || self.records != current {
            let last = pages.len().saturating_sub(1);
            for (index, (records, continuations)) in pages.into_iter().enumerate() {
                emit(&json!({
                    "type": if reset { "reset" } else { "state" },
                    "epoch": epoch, "sequence": sequence,
                    "continuity": if reset { "unknown" } else { "current" },
                    "records": records,
                    "removedIds": if index == last { removed.clone() } else { vec![] },
                    "continuations": continuations,
                }))?;
            }
        }
        self.records = current;
        Ok(())
    }
}

fn record_id(entry: &Value) -> anyhow::Result<String> {
    ["node", "thread", "message"]
        .iter()
        .find_map(|key| entry[*key]["id"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("document entry has no record identity"))
}

pub(crate) fn emit(value: &Value) -> anyhow::Result<()> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, value)?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}
