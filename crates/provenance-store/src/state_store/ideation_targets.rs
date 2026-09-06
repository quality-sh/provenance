//! The write-time target check for contributions and synthesis packets,
//! for the direct writers and the landed batch alike. A target the stored
//! record of the same id already carries is not new: an old dangling
//! target is reported by the gap pass and refuses nothing, so a rewrite
//! that keeps it goes through.

use super::canonical_artifacts::CanonicalArtifactIndex;
use super::{IdeationLandingBatch, StateStore};
use provenance_core::{Contribution, IdeationTarget, ScopeId, StableId, SynthesisPacket};

impl CanonicalArtifactIndex {
    pub(super) fn ensure_new_target_exists(
        &self,
        stored: Option<&IdeationTarget>,
        target: &IdeationTarget,
    ) -> anyhow::Result<()> {
        if stored == Some(target) {
            return Ok(());
        }
        self.ensure_target_exists(target)
    }
}

impl StateStore {
    pub(super) fn ensure_contribution_target(
        &self,
        scope: &ScopeId,
        id: &StableId,
        target: &IdeationTarget,
    ) -> anyhow::Result<()> {
        let stored = self.list_contributions(scope)?;
        self.canonical_artifact_index(scope)?
            .ensure_new_target_exists(contribution_target(&stored, id), target)
    }

    pub(super) fn ensure_synthesis_target(
        &self,
        scope: &ScopeId,
        id: &StableId,
        target: &IdeationTarget,
    ) -> anyhow::Result<()> {
        let stored = self.list_synthesis_packets(scope)?;
        self.canonical_artifact_index(scope)?
            .ensure_new_target_exists(synthesis_target(&stored, id), target)
    }

    /// Every incoming record of a landed batch, against the stored record
    /// of the same id.
    pub(super) fn ensure_batch_targets_exist(
        &self,
        scope: &ScopeId,
        stored_contributions: &[Contribution],
        stored_packets: &[SynthesisPacket],
        incoming: &IdeationLandingBatch,
    ) -> anyhow::Result<()> {
        let index = self.canonical_artifact_index(scope)?;
        for contribution in &incoming.contributions {
            index.ensure_new_target_exists(
                contribution_target(stored_contributions, &contribution.id),
                &contribution.target,
            )?;
        }
        for packet in &incoming.synthesis_packets {
            index.ensure_new_target_exists(
                synthesis_target(stored_packets, &packet.id),
                &packet.target,
            )?;
        }
        Ok(())
    }
}

fn contribution_target<'a>(
    records: &'a [Contribution],
    id: &StableId,
) -> Option<&'a IdeationTarget> {
    records
        .iter()
        .find(|record| record.id == *id)
        .map(|record| &record.target)
}

fn synthesis_target<'a>(
    records: &'a [SynthesisPacket],
    id: &StableId,
) -> Option<&'a IdeationTarget> {
    records
        .iter()
        .find(|record| record.id == *id)
        .map(|record| &record.target)
}
