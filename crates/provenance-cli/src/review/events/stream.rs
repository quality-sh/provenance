//! Current document reconciliation, using the canonical bounded document read.
use super::frames::Frame;
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
        let repository = context.open()?;
        loop {
            let page = queries::read_document(
                &repository,
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
            if cursor.is_none() {
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

/// The client state of one review subscription: the delivered baseline and the last host
/// position. The CLI follow stream and the host delivery task use the same step.
pub(in crate::review) struct Follower {
    baseline: Baseline,
    epoch: String,
    sequence: u64,
    initialized: bool,
}

impl Follower {
    pub fn new(epoch: &str) -> Self {
        Self {
            baseline: Baseline::default(),
            epoch: epoch.to_owned(),
            sequence: 0,
            initialized: false,
        }
    }

    /// The last host position, for a disconnected or read-error frame.
    pub fn position(&self) -> (String, u64) {
        (self.epoch.clone(), self.sequence)
    }

    /// Reads current state for one host frame and returns the frames to deliver.
    pub async fn step(
        &mut self,
        context: &RepoContext,
        root: &str,
        limit: usize,
        frame: &Frame,
    ) -> anyhow::Result<Vec<OutputFrame>> {
        let (next_epoch, next_sequence) = frame.position();
        let reset = self.initialized
            && (matches!(frame, Frame::Reset { .. })
                || next_epoch != self.epoch
                || next_sequence != self.sequence + 1);
        next_epoch.clone_into(&mut self.epoch);
        self.sequence = next_sequence;
        if self.initialized
            && !reset
            && matches!(frame, Frame::Changed { .. } | Frame::Ready { .. })
        {
            return Ok(Vec::new());
        }
        let selected = match frame {
            Frame::Reviewed { scopes, .. } => Some(
                scopes
                    .iter()
                    .filter(|scope| scope.scope == context.scope)
                    .flat_map(|scope| scope.records.iter().map(|record| record.id.clone()))
                    .collect::<BTreeSet<_>>(),
            ),
            _ => None,
        };
        let position = (self.epoch.as_str(), self.sequence);
        let frames = self
            .baseline
            .reconcile(context, root, limit, position, reset, selected.as_ref())
            .await?;
        self.initialized = true;
        Ok(frames)
    }

    /// Reads the first full document at the Ready position. A page write that lands during
    /// the read moves the native sequence or makes a later page refuse; then the read runs
    /// again, at most three times, and after that the document goes out as a reset. The
    /// moved frames stay queued, so later steps also deliver their records.
    #[rule("rule_review_notify_consistent_start")]
    pub async fn start(
        &mut self,
        context: &RepoContext,
        root: &str,
        limit: usize,
        ready: &Frame,
        moved: impl Fn() -> bool,
    ) -> anyhow::Result<Vec<OutputFrame>> {
        for _ in 0..3 {
            *self = Self::new(&self.epoch);
            if let Ok(frames) = self.step(context, root, limit, ready).await {
                if !moved() {
                    return Ok(frames);
                }
            }
        }
        let (epoch, sequence) = ready.position();
        *self = Self::new(epoch);
        self.sequence = sequence;
        self.initialized = true;
        self.baseline
            .reconcile(context, root, limit, (epoch, sequence), true, None)
            .await
    }
}

fn record_id(entry: &Value) -> anyhow::Result<String> {
    ["node", "thread", "message"]
        .iter()
        .find_map(|key| entry[*key]["id"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("document entry has no record identity"))
}
