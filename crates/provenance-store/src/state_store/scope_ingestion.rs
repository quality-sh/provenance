use super::{serde_name, StateStore};
use crate::cache::ProjectionFamily;
use crate::jsonl::write_jsonl_atomic_under_publication;
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
            /// method does not acquire that lock. The caller must remove the old scope directory and
            /// apply the freeze, STE, and repository checks before it publishes the staged state.
            ///
            /// This method does not write the manifest, requirement reviews, the review journal, or
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
                    shards.sources.iter().map(|record| &record.id)
                        .chain(shards.requirements.iter().map(|record| &record.id))
                        .chain(shards.resolutions.iter().map(|record| &record.id))
                        .chain(shards.rules.iter().map(|record| &record.id))
                        .chain(shards.topics.iter().map(|record| &record.id))
                        .chain(shards.questions.iter().map(|record| &record.id))
                        .chain(shards.domains.iter().map(|record| &record.id))
                        .chain(shards.boundaries.iter().map(|record| &record.id)),
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

define_scope_shards! {
    sources: Source => Sources,
    domains: Domain => Domains,
    requirements: Requirement => Requirements,
    boundaries: Boundary => Boundaries,
    topics: Topic => Topics,
    questions: Question => Questions,
    resolutions: Resolution => Resolutions,
    rules: Rule => Rules,
    verification_bindings: VerificationBinding => VerificationBindings,
    implementation_bindings: ImplementationBinding => ImplementationBindings,
    threads: Thread => Threads,
    messages: Message => Messages,
    contributions: Contribution => Contributions,
    synthesis_packets: SynthesisPacket => SynthesisPackets,
    proposal_cards: ProposalCard => ProposalCards,
    assertion_records: AssertionRecord => AssertionRecords,
    dispositions: DispositionRecord => Dispositions,
}

/// Refuses the import when any resource record exceeds the read budget the
/// supported resource reads enforce. Implementation bindings stay outside
/// this check: they are a scanner index, not a served resource.
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
