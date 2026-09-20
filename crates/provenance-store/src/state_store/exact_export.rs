use super::{
    readers::{
        read_ideation_landings_closed, read_jsonl_closed, read_legacy_dispositions_closed,
        read_message_shards_closed,
    },
    IdeationLandingBatch, ScopeId, StateStore,
};
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, ProposalCard, SynthesisPacket, Thread,
};

impl StateStore {
    /// Checks that a typed scope export can represent each open record exactly.
    ///
    /// The export document is closed. An extension field cannot enter that
    /// document and cannot survive a later import, so export refuses the field
    /// before it converts the scope to typed records.
    pub fn ensure_scope_exportable(&self, scope: &ScopeId) -> anyhow::Result<()> {
        drop(self.closed_sources(scope)?);
        drop(self.closed_domains(scope)?);
        drop(self.closed_requirements(scope)?);
        drop(self.closed_boundaries(scope)?);
        drop(self.closed_topics(scope)?);
        drop(self.closed_questions(scope)?);
        drop(self.closed_resolutions(scope)?);
        drop(self.closed_rules(scope)?);
        drop(self.closed_verification_bindings(scope)?);
        drop(self.closed_implementation_bindings(scope)?);
        let threads: Vec<Thread> =
            read_jsonl_closed(self, &crate::shards::threads_path(&self.layout, scope))?;
        drop(threads);
        drop(read_message_shards_closed(self, &self.layout, scope)?);
        read_closed::<Contribution>(
            self,
            &crate::shards::contributions_path(&self.layout, scope),
        )?;
        read_closed::<SynthesisPacket>(
            self,
            &crate::shards::synthesis_packets_path(&self.layout, scope),
        )?;
        read_closed::<ProposalCard>(
            self,
            &crate::shards::proposal_cards_path(&self.layout, scope),
        )?;
        read_closed::<AssertionRecord>(
            self,
            &crate::shards::assertion_records_path(&self.layout, scope),
        )?;
        read_closed::<DispositionRecord>(
            self,
            &crate::shards::dispositions_path(&self.layout, scope),
        )?;
        drop(read_legacy_dispositions_closed(
            self,
            &crate::shards::legacy_promotion_decisions_path(&self.layout, scope),
        )?);
        let landings: Vec<IdeationLandingBatch> = read_ideation_landings_closed(
            self,
            &crate::shards::ideation_landings_path(&self.layout, scope),
        )?;
        drop(landings);
        Ok(())
    }
}

fn read_closed<T: serde::de::DeserializeOwned>(
    store: &StateStore,
    path: &camino::Utf8Path,
) -> anyhow::Result<()> {
    drop(read_jsonl_closed::<T>(store, path)?);
    Ok(())
}
