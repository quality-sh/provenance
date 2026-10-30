mod access;
mod archive_history;
mod canonical_artifacts;
mod domain_writers;
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
pub(crate) mod readers;
pub(crate) mod record_stamps;
mod reference_methods;
mod reference_writers;
mod requirement_reviews;
mod resolution_decisions;
mod rule_writers;
mod scope_ingestion;
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
pub use archive_history::{ArchiveCommitGap, ArchiveCommitGapReason};
pub use ideation_batches::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, CONTRIBUTION_KIND,
    SYNTHESIS_KIND,
};
pub use ideation_edits::{UpdateContributionInput, UpdateSynthesisPacketInput};
pub use inputs::{
    AddSourceReferenceInput, BeginVerificationInput, CascadedResource, CompleteVerificationInput,
    CreateAssertionInput, CreateBoundaryInput, CreateContributionInput, CreateDispositionInput,
    CreateDomainInput, CreateProposalCardInput, CreateQuestionInput, CreateRequirementInput,
    CreateResolutionInput, CreateRuleInput, CreateSourceInput, CreateSynthesisPacketInput,
    CreateTopicInput, DeclarationReferenceInput, MaterializeImplementationBindingInput,
    MaterializeVerificationBindingInput, PostMessageInput, ReconcileState, ReconciledResource,
    TypedAdoptionTarget, TypedDeclarationKind, TypedFieldChange, TypedImplementationInput,
    TypedRequirementInput, TypedResourceKind, TypedRuleInput, TypedSourceInput,
    TypedSpecDiagnostic, TypedSpecInput, TypedSpecResult, UpdateQuestionInput,
};
pub use proposal_surfaces::{ProposalDemand, ProposalSurfaceReason, SurfacedProposal, TopicClaim};
pub use requirement_reviews::{
    requirement_statement_changes, RequirementReviewInput, RequirementStatementChange,
};
pub use scope_ingestion::ScopeShards;
pub(crate) use statement_policy::StatementWriteError;
pub use typed_statement_policy::TypedSpecWriteError;

use crate::{layout::ProvenanceLayout, repository::RepositoryIdentity, shards};
use ideation_batches::overlay_records;
use provenance_core::{
    ensure_record_id_assignable, ensure_supported_schema_version, AssertionRecord, Contribution,
    DispositionRecord, ImplementationBinding, Manifest, Message, ProposalCard, SchemaVersion,
    Scope, ScopeId, SynthesisPacket, Thread, VerificationBinding,
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
use readers::{
    deserialize_closed, read_ideation_landings, read_jsonl, read_jsonl_unlocked,
    read_legacy_dispositions, read_message_shards_unlocked,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProjection {
    schema_version: SchemaVersion,
    scopes: Vec<serde_json::Value>,
    #[serde(default, rename = "disposition_actor_ids")]
    _disposition_actor_ids: Vec<String>,
}

#[derive(Debug)]
enum Access {
    /// Takes the publication lock for each read and write.
    Lock,
    /// Holds the live section: inside a held section, or under a guard. A
    /// store from the repository of a section guard keeps `until`, and its
    /// reads and writes refuse after that section ends.
    Section {
        until: Option<crate::publication::SectionHandle>,
    },
    /// A store over a staged state root. Each read and write takes the lock
    /// that the stage owns.
    Staged {
        permissions: crate::publication::WritePermissions,
        stage: std::sync::Arc<crate::publication::StageLock>,
    },
    /// The store that a staged section hands its closure. Only a staged store
    /// carries write permissions.
    StagedSection {
        permissions: crate::publication::WritePermissions,
    },
    /// Reads with no lock and refuses every write.
    ReadOnly,
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
            crate::test_probes::record_read(&path);
            manifest_from_bytes(&std::fs::read(&path)?).map_err(|error| {
                if error.is::<serde_json::Error>() {
                    error.context(format!("failed to parse manifest {path}"))
                } else {
                    error
                }
            })
        })
    }

    pub(crate) fn closed_manifest_scope(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<(SchemaVersion, Option<Scope>)> {
        self.with_repository_read(|held| {
            let manifest: ManifestProjection =
                deserialize_closed(&std::fs::read_to_string(held.layout.manifest_path())?)?;
            let selected = manifest
                .scopes
                .into_iter()
                .find(|candidate| {
                    candidate.get("id").and_then(serde_json::Value::as_str) == Some(scope.as_str())
                })
                .map(|candidate| deserialize_closed(&serde_json::to_string(&candidate)?))
                .transpose()?;
            Ok((manifest.schema_version, selected))
        })
    }

    pub fn list_scope_directories(&self) -> anyhow::Result<Vec<String>> {
        self.with_repository_read(|held| {
            let scopes_dir = held.layout.scopes_dir();
            if !scopes_dir.exists() {
                return Ok(Vec::new());
            }

            let mut scope_directories = Vec::new();
            for entry in std::fs::read_dir(scopes_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    scope_directories.push(entry.file_name().into_string().map_err(|name| {
                        anyhow::anyhow!(
                            "non-UTF-8 scope directory name: {}",
                            name.to_string_lossy()
                        )
                    })?);
                }
            }
            scope_directories.sort();
            Ok(scope_directories)
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
            let mut records = read_jsonl(held, &shards::contributions_path(&held.layout, scope))?;
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
            let mut records = read_jsonl(held, &shards::proposal_cards_path(&held.layout, scope))?;
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
    let manifest: Manifest = serde_json::from_slice(bytes)?;
    ensure_supported_schema_version("manifest", manifest.schema_version)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests;
