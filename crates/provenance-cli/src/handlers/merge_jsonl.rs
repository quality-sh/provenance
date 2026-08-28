use crate::output;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RbacClaim, RbacSection};
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::merge::{
    changed_statement_diagnostics, merge_records, read_jsonl_records_for_shard,
    validate_merged_records, MergeOutcome,
};
use provenance_store::statement_analysis::violation_error;

/// Merges one JSONL shard and, when asked, writes the result.
///
/// `shard_path` is the repository path the merged result belongs at, which is
/// what tells the merge what type of record the file holds. Git hands a merge
/// driver three temporary files and the real path separately (`%P`), so the
/// caller must pass it; without it the merged records cannot be validated.
///
/// On an rbac-managed repository — decided by the manifest of the repository
/// the driver runs in — the merge demands the claim carried in by the
/// configured driver command's literal `--actor-id <id>` argument, types every
/// merged record against its shard family, and refuses path-less output, so
/// no unchecked family survives. Exiting non-zero leaves the path unmerged
/// for a human.
pub(super) fn handle(
    base: &Utf8PathBuf,
    ours: &Utf8PathBuf,
    theirs: &Utf8PathBuf,
    output_path: Option<Utf8PathBuf>,
    shard_path: Option<&Utf8Path>,
    actor_id: Option<String>,
    format: crate::output::OutputFormat,
) -> anyhow::Result<()> {
    let claim: Option<RbacClaim> = actor_id
        .filter(|id| !id.trim().is_empty())
        .map(RbacClaim::new)
        .transpose()?;
    let rbac = rbac_section()?;
    if rbac.is_some() {
        anyhow::ensure!(
            shard_path.is_some(),
            "rbac: a merge on an rbac-managed repository requires --path; \
             the result cannot be validated without a shard family"
        );
        anyhow::ensure!(
            claim.is_some(),
            "rbac: no actor claim supplied for a mutating operation on an rbac-managed repository"
        );
    }
    let target_path = shard_path.or(output_path.as_deref());
    let base_records = target_path.map_or_else(
        || provenance_store::merge::read_jsonl_records(base),
        |target| read_jsonl_records_for_shard(base, target),
    )?;
    let our_records = target_path.map_or_else(
        || provenance_store::merge::read_jsonl_records(ours),
        |target| read_jsonl_records_for_shard(ours, target),
    )?;
    let their_records = target_path.map_or_else(
        || provenance_store::merge::read_jsonl_records(theirs),
        |target| read_jsonl_records_for_shard(theirs, target),
    )?;
    let outcome = merge_records(&base_records, &our_records, &their_records)?;
    let records = match &outcome {
        MergeOutcome::Clean { records } => records,
        MergeOutcome::Conflicted { partial, .. } => partial,
    };
    if let Some(shard_path) = target_path {
        if let Some(section) = &rbac {
            provenance_store::merge::validate_rbac_merged_records(
                shard_path,
                records,
                section,
                claim.as_ref().expect("claim ensured above"),
            )?;
        }
        validate_merged_records(shard_path, records)?;
        if let Err(error) = ensure_changed_statements_are_clean(shard_path, &base_records, records)
        {
            if matches!(outcome, MergeOutcome::Conflicted { .. }) {
                output::print(format, &outcome)?;
            }
            return Err(error);
        }
    }
    if let Some(output_path) = output_path {
        provenance_store::jsonl::write_jsonl_atomic(&output_path, records)?;
    }
    output::print(format, &outcome)?;
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

/// The repository's grants, if the driver runs inside an rbac-managed
/// repository. Git starts merge drivers at the top of the working tree, so
/// the manifest of the checkout is the one that decides the regime.
///
/// This is a manifest reader, so it runs the same ambiguity law as the other
/// readers before the section is consulted.
fn rbac_section() -> anyhow::Result<Option<RbacSection>> {
    let layout = ProvenanceLayout::new(Utf8PathBuf::from("."));
    let path = layout.manifest_path();
    if !path.exists() {
        return Ok(None);
    }
    let manifest: Manifest = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    provenance_core::ensure_unambiguous_rbac(
        &manifest.disposition_actor_ids,
        manifest.rbac.as_ref(),
    )?;
    Ok(manifest.rbac)
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
