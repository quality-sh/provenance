use super::{
    ensure_new_ids_assignable, overlay_records, read_jsonl_unlocked, read_message_shards_unlocked,
    serde_name, IdeationLandingBatch, StateStore,
};
use crate::cache::ProjectionFamily;
use crate::jsonl::write_jsonl_atomic_under_publication;
use crate::shards;
use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Message, NodeType, ProposalCard, Question, Requirement, Resolution, Rule, ScopeId, Source,
    SynthesisPacket, Thread, ThreadStatus, Topic, VerificationBinding,
};

macro_rules! define_scope_shards {
    ($($field:ident: $record:ty => $family:ident),+ $(,)?) => {
        /// The complete canonical shard contents for one scope import.
        ///
        /// Each slice replaces one shard. An empty slice creates an empty shard.
        #[derive(Default)]
        pub struct ScopeShards<'a> {
            $(pub $field: &'a [$record],)+
        }

        impl StateStore {
            /// Writes the canonical record shards for one scope.
            ///
            /// The caller must hold the publication lock for the complete staged transaction. This
            /// method does not acquire that lock. The caller must apply the freeze, STE, and
            /// repository checks before it publishes the staged state.
            ///
            /// This method replaces the scope directory. It writes the canonical record shards,
            /// but it does not restore the manifest, requirement reviews, review journal, or
            /// ideation landings.
            pub fn import_scope(
                &self,
                scope: &ScopeId,
                shards: &ScopeShards<'_>,
            ) -> anyhow::Result<()> {
                validate_threads(shards.threads)?;
                ensure_import_budgets(shards)?;
                self.ensure_import_ids_unique(
                    scope,
                    shards.sources.iter().map(|record| (NodeType::Source, &record.id))
                        .chain(shards.requirements.iter().map(|record| (NodeType::Requirement, &record.id)))
                        .chain(shards.resolutions.iter().map(|record| (NodeType::Resolution, &record.id)))
                        .chain(shards.rules.iter().map(|record| (NodeType::Rule, &record.id)))
                        .chain(shards.topics.iter().map(|record| (NodeType::Topic, &record.id)))
                        .chain(shards.questions.iter().map(|record| (NodeType::Question, &record.id)))
                        .chain(shards.domains.iter().map(|record| (NodeType::Domain, &record.id)))
                        .chain(shards.boundaries.iter().map(|record| (NodeType::Boundary, &record.id))),
                    &[
                        NodeType::Source,
                        NodeType::Requirement,
                        NodeType::Resolution,
                        NodeType::Rule,
                        NodeType::Topic,
                        NodeType::Question,
                        NodeType::Domain,
                        NodeType::Boundary,
                    ],
                )?;
                ensure_new_ids_assignable(
                    &read_jsonl_unlocked(&shards::verification_bindings_path(&self.layout, scope))?,
                    shards.verification_bindings,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &read_jsonl_unlocked(&shards::implementation_bindings_path(
                        &self.layout,
                        scope,
                    ))?,
                    shards.implementation_bindings,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &read_jsonl_unlocked(&shards::threads_path(&self.layout, scope))?,
                    shards.threads,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &read_message_shards_unlocked(&self.layout, scope)?,
                    shards.messages,
                    |record| record.id.as_str(),
                )?;
                let ideation = read_ideation_records_unlocked(self, scope)?;
                ensure_new_ids_assignable(
                    &ideation.contributions,
                    shards.contributions,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &ideation.synthesis_packets,
                    shards.synthesis_packets,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &ideation.proposals,
                    shards.proposal_cards,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &ideation.assertions,
                    shards.assertion_records,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &ideation.dispositions,
                    shards.dispositions,
                    |record| record.id.as_str(),
                )?;
                let scope_dir = self.layout.scopes_dir().join(scope.as_str());
                if scope_dir.exists() {
                    std::fs::remove_dir_all(&scope_dir)?;
                }
                $(
                    write_jsonl_atomic_under_publication(
                        &ProjectionFamily::$family.shard_path(&self.layout, scope),
                        shards.$field,
                    )?;
                )+
                Ok(())
            }
        }
    };
}

macro_rules! define_imported_scope_shards {
    (
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        bindings { $($binding_variant:ident: $binding_type:ty, $binding_field:ident, $binding_path:ident, $binding_suffix:literal, $binding_table:literal, [$($binding_node:tt)*], $binding_reader:ident, [$($binding_closed:tt)*], $binding_id:ident, [$($binding_loader:tt)*], [$($binding_catalog:tt)*];)* }
        internal { $($internal:tt)* }
    ) => {
        define_scope_shards! {
            $($export_field: $export_type => $export_variant,)*
            $($canonical_field: $canonical_type => $canonical_variant,)*
            $($binding_field: $binding_type => $binding_variant,)*
        }
    };
}

crate::cache::record_families!(define_imported_scope_shards);

struct StoredIdeationRecords {
    contributions: Vec<Contribution>,
    synthesis_packets: Vec<SynthesisPacket>,
    proposals: Vec<ProposalCard>,
    assertions: Vec<AssertionRecord>,
    dispositions: Vec<DispositionRecord>,
}

fn read_ideation_records_unlocked(
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<StoredIdeationRecords> {
    let layout = &store.layout;
    let mut records = StoredIdeationRecords {
        contributions: read_jsonl_unlocked(&shards::contributions_path(layout, scope))?,
        synthesis_packets: read_jsonl_unlocked(&shards::synthesis_packets_path(layout, scope))?,
        proposals: read_jsonl_unlocked(&shards::proposal_cards_path(layout, scope))?,
        assertions: read_jsonl_unlocked(&shards::assertion_records_path(layout, scope))?,
        dispositions: read_jsonl_unlocked(&shards::dispositions_path(layout, scope))?,
    };
    records
        .dispositions
        .extend(super::readers::read_legacy_dispositions_unlocked(
            &shards::legacy_promotion_decisions_path(layout, scope),
        )?);
    let landings: Vec<IdeationLandingBatch> = super::readers::read_ideation_landings_unlocked(
        &shards::ideation_landings_path(layout, scope),
    )?;
    for batch in landings {
        overlay_records(&mut records.contributions, batch.contributions, |r| {
            r.id.as_str()
        });
        overlay_records(
            &mut records.synthesis_packets,
            batch.synthesis_packets,
            |r| r.id.as_str(),
        );
        overlay_records(&mut records.proposals, batch.proposals, |r| r.id.as_str());
        overlay_records(&mut records.assertions, batch.assertions, |r| r.id.as_str());
        overlay_records(&mut records.dispositions, batch.dispositions, |r| {
            r.id.as_str()
        });
    }
    Ok(records)
}

/// Refuses an import with a resource record that exceeds its read budget.
/// Implementation bindings are a scanner index, not a served resource.
fn ensure_import_budgets(shards: &ScopeShards<'_>) -> anyhow::Result<()> {
    use super::read_budget::ensure_slice_within_read_budget as budget;
    budget(shards.sources)?;
    budget(shards.domains)?;
    budget(shards.requirements)?;
    budget(shards.boundaries)?;
    budget(shards.topics)?;
    budget(shards.questions)?;
    budget(shards.resolutions)?;
    budget(shards.rules)?;
    budget(shards.verification_bindings)?;
    budget(shards.threads)?;
    budget(shards.messages)?;
    budget(shards.contributions)?;
    budget(shards.synthesis_packets)?;
    budget(shards.proposal_cards)?;
    budget(shards.assertion_records)?;
    budget(shards.dispositions)?;
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
            let node_type = serde_name(&thread.parent.node_type)?;
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
