mod access;
mod archive_history;
mod canonical_artifacts;
mod family_readers;
mod graph_validation;
mod ideation_batches;
mod ideation_edits;
mod ideation_targets;
mod ideation_validation;
mod ideation_writers;
mod implementation_bindings;
mod inputs;
mod live_parents;
pub(crate) mod message_ids;
mod proposal_surfaces;
mod proposal_writers;
pub(crate) mod read_budget;
mod read_fence;
pub(crate) mod readers;
pub(crate) mod record_stamps;
mod reference_methods;
mod reference_writers;
mod requirement_reviews;
mod resolution_decisions;
mod rule_writers;
mod scope_ingestion;
mod shared_parses;
pub(crate) use shared_parses::with_shared_parses;
mod shaping_writers;
mod statement_policy;
mod thread_origins;
mod thread_writers;
pub(super) mod typed_specs;
mod typed_statement_policy;
mod updates;
mod verification_bindings;
mod verification_runs;
mod withdrawal_writers;
mod writers;
pub use updates::*;

pub use access::GuardedStore;
pub(crate) use access::ViewStore;
pub(crate) use access::{SectionView, StateEntry};
pub use archive_history::{ArchiveCommitGap, ArchiveCommitGapReason};
pub use graph_validation::validate_relations;
pub use ideation_batches::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, CONTRIBUTION_KIND,
    SYNTHESIS_KIND,
};
pub use ideation_edits::{UpdateContributionInput, UpdateSynthesisPacketInput};
pub use inputs::{
    AddSourceReferenceInput, BeginVerificationInput, CascadedResource, CompleteVerificationInput,
    CreateAssertionInput, CreateContributionInput, CreateDispositionInput, CreateProposalCardInput,
    CreateQuestionInput, CreateRequirementInput, CreateResolutionInput, CreateRuleInput,
    CreateSourceInput, CreateSynthesisPacketInput, CreateTopicInput, DeclarationReferenceInput,
    MaterializeImplementationBindingInput, MaterializeVerificationBindingInput, PostMessageInput,
    ReconcileState, ReconciledResource, TypedAdoptionTarget, TypedDeclarationKind,
    TypedFieldChange, TypedImplementationInput, TypedRequirementInput, TypedResourceKind,
    TypedRuleInput, TypedSourceInput, TypedSpecDiagnostic, TypedSpecInput, TypedSpecResult,
    UpdateQuestionInput,
};
pub use proposal_surfaces::{ProposalDemand, ProposalSurfaceReason, SurfacedProposal, TopicClaim};
pub(crate) use read_fence::FencedStore;
pub use requirement_reviews::{
    requirement_statement_changes, RequirementReviewInput, RequirementStatementChange,
};
pub use scope_ingestion::ScopeShards;
pub(crate) use statement_policy::StatementWriteError;
pub use typed_specs::{decide_typed_spec, TypedSpecDecision, TypedSpecView};
pub use typed_statement_policy::TypedSpecWriteError;

use crate::{layout::ProvenanceLayout, repository::RepositoryIdentity, shards};
use ideation_batches::overlay_records;
use provenance_core::{
    ensure_record_id_assignable, AssertionRecord, Contribution, DispositionRecord,
    ImplementationBinding, Manifest, Message, ProposalCard, ScopeId, SynthesisPacket, Thread,
    VerificationBinding,
};

fn ensure_new_ids_assignable<T>(
    existing: &[T],
    incoming: &[T],
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    for record in incoming {
        let assigned = id(record);
        if !existing.iter().any(|known| id(known) == assigned) {
            ensure_record_id_assignable(assigned)?;
        }
    }
    Ok(())
}
use readers::{read_ideation_landings, read_jsonl_unlocked, read_message_shards_unlocked};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
enum Access {
    /// Takes the publication lock for each read and write.
    Lock,
    /// Holds the live section: inside a held section, or under a guard. A
    /// store from the repository of a section guard keeps `until`, and its
    /// reads and writes refuse after that section ends. A store of an opened
    /// repository reads and writes through the `view` of the section; a
    /// layout-level store has none and reads the files.
    Section {
        until: Option<crate::publication::SectionHandle>,
        view: Option<std::sync::Arc<access::SectionView>>,
    },
    /// A store over a stage: it reads the overlay of the stage, and each
    /// write lands there. Each read and write takes the lock that the stage
    /// owns.
    Staged {
        permissions: crate::publication::WritePermissions,
        stage: std::sync::Arc<crate::publication::StageLock>,
    },
    /// The store that a staged section hands its closure. Only a staged store
    /// carries write permissions.
    StagedSection {
        permissions: crate::publication::WritePermissions,
        stage: std::sync::Arc<crate::publication::StageLock>,
    },
    /// Reads with no lock and refuses every write.
    ReadOnly,
    /// Reads one view of canonical state with no lock and no disk access,
    /// and refuses every write and stage. A fenced view store reads only
    /// below the directories of its fence.
    View(read_fence::ViewReads),
}

#[derive(Clone, Debug)]
pub(crate) enum RecordStamp {
    ResolveFromRepository,
    Omit,
    Fixed(provenance_core::Stamp),
}

impl RecordStamp {
    /// The stamp a write resolved once before it staged: that stamp, or none.
    pub(crate) fn resolved(stamp: Option<provenance_core::Stamp>) -> Self {
        stamp.map_or(Self::Omit, Self::Fixed)
    }
}

/// A store over one state root.
///
/// A store from [`Repository::store`](crate::repository::Repository::store) or
/// [`Repository::stage`](crate::repository::Repository::stage) carries the live
/// identity; a layout-level store from `init` or a repository-wide `check`
/// has none. A store has no `Clone`, so a staged store exists only as the
/// stage body's borrow.
#[derive(Debug)]
pub struct StateStore {
    identity: Option<RepositoryIdentity>,
    pub(crate) layout: ProvenanceLayout,
    access: Access,
    record_stamp: RecordStamp,
    storage: Storage,
}

/// The adapter that a store publishes through.
#[derive(Clone, Debug)]
enum Storage {
    /// A local-disk adapter over the layout of the store, made at each use.
    /// A layout-level store and a staged store use it.
    Layout,
    /// The adapter of the opened repository and the lock of its section.
    Shared {
        storage: std::sync::Arc<dyn crate::storage::StateStorage>,
        lock: std::sync::Arc<dyn crate::storage::SectionLock>,
        /// Whether the adapter is the local adapter of the layout of the
        /// store, so that a working-tree file can land with the state.
        local: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeationLandingBatch {
    #[serde(default)]
    pub contributions: Vec<Contribution>,
    #[serde(default)]
    pub synthesis_packets: Vec<SynthesisPacket>,
    #[serde(default)]
    pub proposals: Vec<ProposalCard>,
    #[serde(default)]
    pub assertions: Vec<AssertionRecord>,
    #[serde(default)]
    pub dispositions: Vec<DispositionRecord>,
}

#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PostMessageResult {
    pub thread: Thread,
    pub message: Message,
}

impl StateStore {
    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        self.with_repository_read(|held| {
            let path = held.layout.manifest_path();
            manifest_from_bytes(&held.read_state(&path)?).map_err(|error| {
                if error.is::<serde_json::Error>() {
                    error.context(format!("failed to parse manifest {path}"))
                } else {
                    error
                }
            })
        })
    }

    pub fn list_scope_directories(&self) -> anyhow::Result<Vec<String>> {
        self.with_repository_read(|held| {
            held.state_keys_under(&held.layout.scopes_dir(), StateEntry::Directory, |path| {
                anyhow::anyhow!(
                    "non-UTF-8 scope directory name: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )
            })
        })
    }

    pub fn active_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        Ok(self
            .list_verification_bindings(scope)?
            .into_iter()
            .collect())
    }
    pub fn active_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        Ok(self
            .list_implementation_bindings(scope)?
            .into_iter()
            .collect())
    }
    fn list_contributions_after_direct_read(
        &self,
        scope: &ScopeId,
        after_direct_read: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<Contribution>> {
        self.with_repository_read(|held| {
            let mut records = shared_parses::read_jsonl_shared(
                held,
                &shards::contributions_path(&held.layout, scope),
            )?;
            after_direct_read()?;
            for batch in held.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.contributions, |record| {
                    record.id.as_str()
                });
            }
            Ok(records)
        })
    }
    pub fn list_proposal_cards_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<Vec<ProposalCard>> {
        self.project_proposal_cards(scope, Some(disposition_actor_ids), || Ok(()))
    }
    fn project_proposal_cards(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: Option<&[String]>,
        after_validation: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<ProposalCard>> {
        self.with_repository_read(|held| {
            if let Some(disposition_actor_ids) = disposition_actor_ids {
                held.validate_ideation_scope_with_actor_ids(scope, disposition_actor_ids)?;
            } else {
                held.validate_ideation_scope(scope)?;
            }
            held.validate_graph_scope(scope)?;
            after_validation()?;
            held.proposal_cards_of_validated_scope(scope)
        })
    }
    /// The proposal cards with their effective state, for a caller that
    /// validated the ideation and graph records of `scope` under the lock it
    /// still holds.
    pub(crate) fn proposal_cards_of_validated_scope(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ProposalCard>> {
        self.with_repository_read(|held| {
            let assertions = held.list_assertion_records(scope)?;
            let dispositions = held.list_dispositions(scope)?;
            Ok(held
                .list_proposal_definitions(scope)?
                .into_iter()
                .map(|mut proposal| {
                    proposal.promotion_state = provenance_core::effective_proposal_state(
                        &proposal,
                        &assertions,
                        &dispositions,
                    );
                    proposal
                })
                .collect())
        })
    }
    pub fn list_proposal_definitions(&self, scope: &ScopeId) -> anyhow::Result<Vec<ProposalCard>> {
        self.with_repository_read(|held| {
            let mut records = shared_parses::read_jsonl_shared(
                held,
                &shards::proposal_cards_path(&held.layout, scope),
            )?;
            for batch in held.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.proposals, |record| record.id.as_str());
            }
            Ok(records)
        })
    }
}

pub fn serde_name<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("expected string enum serialization"))?
        .to_string())
}

pub(crate) fn manifest_from_bytes(bytes: &[u8]) -> anyhow::Result<Manifest> {
    Ok(serde_json::from_slice(bytes)?)
}

#[cfg(test)]
mod tests;
