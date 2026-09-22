use super::{ensure_new_ids_assignable, serde_name, StateStore};
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
                ensure_new_ids_assignable(
                    &self.list_verification_bindings(scope)?,
                    shards.verification_bindings,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_implementation_bindings(scope)?,
                    shards.implementation_bindings,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_threads(scope)?,
                    shards.threads,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_messages(scope)?,
                    shards.messages,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_contributions(scope)?,
                    shards.contributions,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_synthesis_packets(scope)?,
                    shards.synthesis_packets,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_proposal_definitions(scope)?,
                    shards.proposal_cards,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_assertion_records(scope)?,
                    shards.assertion_records,
                    |record| record.id.as_str(),
                )?;
                ensure_new_ids_assignable(
                    &self.list_dispositions(scope)?,
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
