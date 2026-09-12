use super::{serde_name, StateStore};
use crate::cache::ProjectionFamily;
use crate::jsonl::write_jsonl_atomic_under_publication;
use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Message, ProposalCard, Question, Requirement, Resolution, Rule, ScopeId, Source,
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
