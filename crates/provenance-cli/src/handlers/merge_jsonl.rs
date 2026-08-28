use crate::output;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{RbacClaim, RbacSection};
use provenance_macros::rule;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::merge::{
    changed_statement_diagnostics, merge_records, read_jsonl_records_for_shard,
    validate_merged_records, MergeOutcome,
};
use provenance_store::state_store::StateStore;
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
    // Preflight regime detection through the canonical store reader. This is
    // a read; the authoritative decision runs inside the publication critical
    // section below, before any byte moves.
    let rbac = rbac_section()?;
    if rbac.is_some() {
        anyhow::ensure!(
            shard_path.is_some(),
            "rbac: a merge on an rbac-managed repository requires --path; \
             the result cannot be validated without a shard family"
        );
        anyhow::ensure!(claim.is_some(), provenance_core::MISSING_CLAIM_REFUSAL);
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
        write_output(
            rbac.as_ref(),
            &output_path,
            shard_path,
            claim.as_ref(),
            records,
        )?;
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

/// The write half of the merge, and with it the authoritative authorization:
/// on an rbac-managed repository the section is re-read through the canonical
/// reader and the typed, authorized validation reruns inside one publication
/// critical section, so the decision and the write resolve against the same
/// manifest bytes no concurrent writer can move. Repositories without an
/// `rbac` section write exactly as before.
fn write_output(
    rbac: Option<&RbacSection>,
    output_path: &Utf8Path,
    shard_path: Option<&Utf8Path>,
    claim: Option<&RbacClaim>,
    records: &[serde_json::Value],
) -> anyhow::Result<()> {
    if rbac.is_none() {
        return provenance_store::jsonl::write_jsonl_atomic(output_path, records);
    }
    let shard_path = shard_path.expect("shard path ensured for the rbac regime");
    let claim = claim.expect("claim ensured for the rbac regime");
    let layout = ProvenanceLayout::new(Utf8PathBuf::from("."));
    let store = StateStore::new(layout.clone());
    provenance_store::publication::with_repository_publication(&layout, || {
        let manifest = store.manifest()?;
        let Some(section) = &manifest.rbac else {
            anyhow::bail!(
                "rbac: the manifest no longer carries the rbac section the merge \
                 was authorized under"
            );
        };
        provenance_store::merge::validate_rbac_merged_records(shard_path, records, section, claim)?;
        provenance_store::jsonl::write_jsonl_atomic(output_path, records)
    })
}

/// The repository's grants, if the driver runs inside an rbac-managed
/// repository. Git starts merge drivers at the top of the working tree, so
/// the manifest of the checkout is the one that decides the regime.
///
/// This is a manifest reader, so it is the canonical store reader: schema
/// check, the ambiguity refusal, and the section well-formedness law all run
/// here, and an unreadable manifest refuses the merge.
fn rbac_section() -> anyhow::Result<Option<RbacSection>> {
    let layout = ProvenanceLayout::new(Utf8PathBuf::from("."));
    if !layout.manifest_path().exists() {
        return Ok(None);
    }
    let manifest = StateStore::new(layout).manifest()?;
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
