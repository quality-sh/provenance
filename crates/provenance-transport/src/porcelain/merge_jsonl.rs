use super::ArtifactPort;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use provenance_porcelain::merge_jsonl::{MergeJsonlInput, MergeJsonlPort, MergeReport};
use provenance_porcelain::WorkFuture;
use provenance_store::merge::{
    changed_statement_diagnostics, merge_records, read_jsonl_rows, read_jsonl_rows_for_shard,
    validate_merged_records, MergeOutcome, ShardFamily, StoredRow,
};
use provenance_store::statement_analysis::violation_error;

/// A failed merge: the error, and the merge outcome when the merge had one to report
/// before the error.
#[derive(Debug)]
pub struct MergeFailure {
    pub outcome: Option<String>,
    pub error: anyhow::Error,
}

impl MergeJsonlPort for ArtifactPort {
    type Failure = MergeFailure;

    fn merge_jsonl(&self, input: MergeJsonlInput) -> WorkFuture<'_, MergeReport, MergeFailure> {
        Box::pin(async move {
            let path = input.path.map(Utf8PathBuf::from);
            merge(
                &input.base.into(),
                &input.ours.into(),
                &input.theirs.into(),
                input.output.map(Utf8PathBuf::from),
                path.as_deref(),
            )
            .unwrap_or_else(|error| {
                Err(MergeFailure {
                    outcome: None,
                    error,
                })
            })
        })
    }
}

/// Merges one JSONL shard and, when asked, writes the result.
///
/// `shard_path` is the repository path the merged result belongs at, which is
/// what tells the merge what type of record the file holds. Git hands a merge
/// driver three temporary files and the real path separately (`%P`), so the
/// caller must pass it; without it the merged records are written unchecked.
///
/// The report names each conflict; the caller fails on a conflict, because a git
/// merge driver that exits zero tells git the file merged cleanly. Exiting
/// non-zero leaves the path unmerged for a human to resolve.
fn merge(
    base: &Utf8PathBuf,
    ours: &Utf8PathBuf,
    theirs: &Utf8PathBuf,
    output_path: Option<Utf8PathBuf>,
    shard_path: Option<&Utf8Path>,
) -> anyhow::Result<Result<MergeReport, MergeFailure>> {
    let target_path = shard_path.or(output_path.as_deref());
    let read_rows = |path: &Utf8PathBuf| {
        target_path.map_or_else(
            || read_jsonl_rows(path),
            |target| read_jsonl_rows_for_shard(path, target),
        )
    };
    let base_rows = read_rows(base)?;
    let our_rows = read_rows(ours)?;
    let their_rows = read_rows(theirs)?;
    let record_values = |rows: &[StoredRow]| {
        rows.iter()
            .map(|row| row.record().clone())
            .collect::<Vec<_>>()
    };
    let key = target_path
        .map_or(ShardFamily::Unrecognized, ShardFamily::for_shard_path)
        .record_key();
    let outcome = merge_records(
        key,
        &record_values(&base_rows),
        &record_values(&our_rows),
        &record_values(&their_rows),
    )?;
    let records = match &outcome {
        MergeOutcome::Clean { records } => records,
        MergeOutcome::Conflicted { partial, .. } => partial,
    };
    if let Some(shard_path) = target_path {
        validate_merged_records(shard_path, records)?;
        if let Err(error) =
            ensure_changed_statements_are_clean(shard_path, &record_values(&base_rows), records)
        {
            let outcome = matches!(outcome, MergeOutcome::Conflicted { .. })
                .then(|| pretty(&outcome))
                .transpose()?;
            return Ok(Err(MergeFailure { outcome, error }));
        }
    }
    if let Some(output_path) = output_path {
        // The writer selects the stored row that matches each merged record,
        // so a caller cannot pair checked records with unrelated raw lines.
        provenance_store::jsonl::write_preserved_jsonl_atomic(
            &output_path,
            &our_rows,
            &their_rows,
            records,
        )?;
    }
    let conflicts = match &outcome {
        MergeOutcome::Clean { .. } => Vec::new(),
        MergeOutcome::Conflicted { conflicts, .. } => conflicts
            .iter()
            .map(|conflict| conflict.record_id.clone())
            .collect(),
    };
    Ok(Ok(MergeReport {
        outcome: pretty(&outcome)?,
        conflicts,
    }))
}

/// The outcome as the pretty JSON document that the merge reports.
fn pretty(outcome: &MergeOutcome) -> serde_json::Result<String> {
    serde_json::to_string_pretty(outcome)
}

#[rule("rule_ste_merge_changed_statement_gate")]
fn ensure_changed_statements_are_clean(
    shard_path: &Utf8Path,
    base: &[serde_json::Value],
    candidate: &[serde_json::Value],
) -> anyhow::Result<()> {
    let diagnostics = changed_statement_diagnostics(shard_path, base, candidate)?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(violation_error(&diagnostics))
    }
}
