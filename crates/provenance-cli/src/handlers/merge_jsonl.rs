use crate::output;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use provenance_store::merge::{
    changed_statement_diagnostics, merge_records, preserved_lines, read_jsonl_rows,
    read_jsonl_rows_for_shard, validate_merged_records, MergeOutcome, StoredRow,
};
use provenance_store::statement_analysis::violation_error;

/// Merges one JSONL shard and, when asked, writes the result.
///
/// `shard_path` is the repository path the merged result belongs at, which is
/// what tells the merge what type of record the file holds. Git hands a merge
/// driver three temporary files and the real path separately (`%P`), so the
/// caller must pass it; without it the merged records are written unchecked.
///
/// Conflicts are reported on stdout and then fail the command, because a git
/// merge driver that exits zero tells git the file merged cleanly. Exiting
/// non-zero leaves the path unmerged for a human to resolve.
pub(super) fn handle(
    base: &Utf8PathBuf,
    ours: &Utf8PathBuf,
    theirs: &Utf8PathBuf,
    output_path: Option<Utf8PathBuf>,
    shard_path: Option<&Utf8Path>,
) -> anyhow::Result<()> {
    let target_path = shard_path.or(output_path.as_deref());
    let read_rows = |path: &Utf8PathBuf| match target_path {
        Some(target) => read_jsonl_rows_for_shard(path, target),
        None => read_jsonl_rows(path),
    };
    let base_rows = read_rows(base)?;
    let our_rows = read_rows(ours)?;
    let their_rows = read_rows(theirs)?;
    let record_values =
        |rows: &[StoredRow]| rows.iter().map(|row| row.record.clone()).collect::<Vec<_>>();
    let outcome = merge_records(
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
            if matches!(outcome, MergeOutcome::Conflicted { .. }) {
                output::print_json(&outcome)?;
            }
            return Err(error);
        }
    }
    if let Some(output_path) = output_path {
        // Every merged record is one side's stored record, so the result is
        // written from the stored lines: untouched rows keep their exact
        // bytes, and an adopted row lands as the side that moved it held it.
        let lines = preserved_lines(&our_rows, &their_rows, records)?;
        provenance_store::jsonl::write_jsonl_lines_atomic(&output_path, records, &lines)?;
    }
    output::print_json(&outcome)?;
    if let MergeOutcome::Conflicted { conflicts, .. } = &outcome {
        anyhow::bail!(
            "merge left {} conflicting record(s): {}",
            conflicts.len(),
            conflicts
                .iter()
                .map(|conflict| conflict.record_id.clone())
                .collect::<Vec<String>>()
                .join(", ")
        );
    }
    Ok(())
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
