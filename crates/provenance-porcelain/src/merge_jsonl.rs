//! Merge JSONL: merge one record shard three ways, as a Git merge driver does.

use crate::WorkFuture;
use serde::Deserialize;

/// The `merge-jsonl` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeJsonlInput {
    pub base: String,
    pub ours: String,
    pub theirs: String,
    pub output: Option<String>,
    pub path: Option<String>,
    pub format: String,
}

/// The result of a merge. When the input names an output file, the port writes the
/// merged records to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeReport {
    /// The merge outcome as one pretty JSON document.
    pub outcome: String,
    /// The ID of each record that the merge left in conflict.
    pub conflicts: Vec<String>,
}

/// The port that merges record shards.
pub trait MergeJsonlPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn merge_jsonl(&self, input: MergeJsonlInput) -> WorkFuture<'_, MergeReport, Self::Failure>;
}

impl<P: MergeJsonlPort> crate::Porcelain<P> {
    /// Merges the three shard files of the input.
    pub async fn merge_jsonl(&self, input: MergeJsonlInput) -> Result<MergeReport, P::Failure> {
        self.port.merge_jsonl(input).await
    }
}
