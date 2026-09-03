use super::ScopeExport;
use provenance_core::{ScopeId, Thread, ThreadStatus};
use provenance_store::layout::ProvenanceLayout;

pub(super) fn write_scope(
    layout: &ProvenanceLayout,
    scope_id: &ScopeId,
    exported: &ScopeExport,
) -> anyhow::Result<()> {
    validate_threads(&exported.threads)?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::sources_path(layout, scope_id),
        &exported.sources,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::domains_path(layout, scope_id),
        &exported.domains,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::requirements_path(layout, scope_id),
        &exported.requirements,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::boundaries_path(layout, scope_id),
        &exported.boundaries,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::topics_path(layout, scope_id),
        &exported.topics,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::questions_path(layout, scope_id),
        &exported.questions,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::resolutions_path(layout, scope_id),
        &exported.resolutions,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::rules_path(layout, scope_id),
        &exported.rules,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::verification_bindings_path(layout, scope_id),
        &exported.verification_bindings,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::implementation_bindings_path(layout, scope_id),
        &exported.implementation_bindings,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::threads_path(layout, scope_id),
        &exported.threads,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::messages_path(layout, scope_id),
        &exported.messages,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::contributions_path(layout, scope_id),
        &exported.contributions,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::synthesis_packets_path(layout, scope_id),
        &exported.synthesis_packets,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::proposal_cards_path(layout, scope_id),
        &exported.proposal_cards,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::assertion_records_path(layout, scope_id),
        &exported.assertion_records,
    )?;
    provenance_store::jsonl::write_jsonl_atomic(
        &provenance_store::shards::dispositions_path(layout, scope_id),
        &exported.dispositions,
    )?;
    Ok(())
}

fn validate_threads(threads: &[Thread]) -> anyhow::Result<()> {
    for (index, thread) in threads.iter().enumerate() {
        if threads[..index]
            .iter()
            .any(|earlier| earlier.id == thread.id)
        {
            anyhow::bail!("duplicate thread id {}", thread.id.as_str());
        }
        if thread.status != ThreadStatus::Active {
            continue;
        }
        if let Some(earlier) = threads[..index].iter().find(|earlier| {
            earlier.status == ThreadStatus::Active && earlier.parent == thread.parent
        }) {
            let node_type = provenance_store::state_store::serde_name(&thread.parent.node_type)?;
            anyhow::bail!(
                "multiple active threads for {} {}: {} and {}",
                node_type,
                thread.parent.node_id.as_str(),
                earlier.id.as_str(),
                thread.id.as_str()
            );
        }
    }
    Ok(())
}
