mod canonical_artifacts;
mod domain_writers;
mod ideation_batches;
mod ideation_writers;
mod implementation_bindings;
mod inputs;
mod proposal_surfaces;
mod proposal_writers;
pub(crate) mod readers;
mod requirement_reviews;
mod rule_writers;
mod shaping_writers;
mod statement_policy;
mod thread_writers;
pub(super) mod typed_specs;
mod typed_statement_policy;
mod verification_bindings;
mod verification_runs;
mod writers;

pub use ideation_batches::{
    assertion_cites_contribution, assertion_cites_synthesis,
    ensure_asserted_contribution_unchanged, ensure_asserted_synthesis_unchanged, CONTRIBUTION_KIND,
    SYNTHESIS_KIND,
};
pub use inputs::{
    AddSourceReferenceInput, BeginVerificationInput, CompleteVerificationInput,
    CreateAssertionInput, CreateBoundaryInput, CreateContributionInput, CreateDispositionInput,
    CreateDomainInput, CreateEdgeInput, CreateProposalCardInput, CreateQuestionInput,
    CreateRequirementInput, CreateResolutionInput, CreateRuleInput, CreateSourceInput,
    CreateSynthesisPacketInput, CreateTopicInput, DeclarationReferenceInput,
    MaterializeImplementationBindingInput, MaterializeVerificationBindingInput, PostMessageInput,
    ReconcileState, ReconciledResource, TypedAdoptionTarget, TypedDeclarationKind,
    TypedFieldChange, TypedImplementationInput, TypedRequirementInput, TypedResourceKind,
    TypedRuleInput, TypedSourceInput, TypedSpecDiagnostic, TypedSpecInput, TypedSpecResult,
    UpdateQuestionInput,
};
pub use proposal_surfaces::{ProposalDemand, ProposalSurfaceReason, SurfacedProposal, TopicClaim};
pub use requirement_reviews::{
    requirement_statement_changes, RequirementReviewInput, RequirementStatementChange,
};
pub use typed_statement_policy::TypedSpecWriteError;

use crate::{layout::ProvenanceLayout, shards};
use ideation_batches::overlay_records;
use provenance_core::{
    ensure_supported_schema_version, AssertionRecord, Boundary, Contribution, DispositionRecord,
    Domain, Edge, ImplementationBinding, Manifest, Message, ProposalCard, Question, Requirement,
    Resolution, Rule, SchemaVersion, Scope, ScopeId, Source, SynthesisPacket, Thread, Topic,
    VerificationBinding,
};
use readers::{
    deserialize_closed, read_edge_shards, read_ideation_landings, read_jsonl, read_jsonl_closed,
    read_legacy_dispositions, read_message_shards,
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

#[derive(Debug, Clone)]
pub struct StateStore {
    pub(crate) layout: ProvenanceLayout,
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
pub struct PostMessageResult {
    pub thread: Thread,
    pub message: Message,
}

impl StateStore {
    pub const fn new(layout: ProvenanceLayout) -> Self {
        Self { layout }
    }
    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        self.with_repository_publication(|| {
            crate::test_probes::record_read(&self.layout.manifest_path());
            let manifest: Manifest =
                serde_json::from_str(&std::fs::read_to_string(self.layout.manifest_path())?)?;
            ensure_supported_schema_version("manifest", manifest.schema_version)?;
            Ok(manifest)
        })
    }

    pub(crate) fn closed_manifest_scope(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<(SchemaVersion, Option<Scope>)> {
        self.with_repository_publication(|| {
            let manifest: ManifestProjection =
                deserialize_closed(&std::fs::read_to_string(self.layout.manifest_path())?)?;
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
        self.with_repository_publication(|| {
            let scopes_dir = self.layout.scopes_dir();
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

    pub fn list_sources(&self, scope: &ScopeId) -> anyhow::Result<Vec<Source>> {
        read_jsonl(&shards::sources_path(&self.layout, scope))
    }
    pub fn list_requirements(&self, scope: &ScopeId) -> anyhow::Result<Vec<Requirement>> {
        read_jsonl(&shards::requirements_path(&self.layout, scope))
    }
    pub fn list_domains(&self, scope: &ScopeId) -> anyhow::Result<Vec<Domain>> {
        read_jsonl(&shards::domains_path(&self.layout, scope))
    }
    pub fn list_boundaries(&self, scope: &ScopeId) -> anyhow::Result<Vec<Boundary>> {
        read_jsonl(&shards::boundaries_path(&self.layout, scope))
    }
    pub fn list_topics(&self, scope: &ScopeId) -> anyhow::Result<Vec<Topic>> {
        read_jsonl(&shards::topics_path(&self.layout, scope))
    }
    pub fn list_questions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Question>> {
        read_jsonl(&shards::questions_path(&self.layout, scope))
    }
    pub fn list_edges(&self) -> anyhow::Result<Vec<Edge>> {
        read_edge_shards(&self.layout, None)
    }
    pub fn list_resolutions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Resolution>> {
        read_jsonl(&shards::resolutions_path(&self.layout, scope))
    }
    pub fn list_rules(&self, scope: &ScopeId) -> anyhow::Result<Vec<Rule>> {
        read_jsonl(&shards::rules_path(&self.layout, scope))
    }
    pub fn list_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        read_jsonl(&shards::verification_bindings_path(&self.layout, scope))
    }
    pub fn active_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        Ok(self
            .list_verification_bindings(scope)?
            .into_iter()
            .filter(|binding| !binding.retired)
            .collect())
    }
    pub fn list_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        read_jsonl(&shards::implementation_bindings_path(&self.layout, scope))
    }
    pub fn active_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        Ok(self
            .list_implementation_bindings(scope)?
            .into_iter()
            .filter(|binding| !binding.retired)
            .collect())
    }
    pub(crate) fn closed_sources(&self, scope: &ScopeId) -> anyhow::Result<Vec<Source>> {
        read_jsonl_closed(&shards::sources_path(&self.layout, scope))
    }
    pub(crate) fn closed_requirements(&self, scope: &ScopeId) -> anyhow::Result<Vec<Requirement>> {
        read_jsonl_closed(&shards::requirements_path(&self.layout, scope))
    }
    pub(crate) fn closed_domains(&self, scope: &ScopeId) -> anyhow::Result<Vec<Domain>> {
        read_jsonl_closed(&shards::domains_path(&self.layout, scope))
    }
    pub(crate) fn closed_boundaries(&self, scope: &ScopeId) -> anyhow::Result<Vec<Boundary>> {
        read_jsonl_closed(&shards::boundaries_path(&self.layout, scope))
    }
    pub(crate) fn closed_topics(&self, scope: &ScopeId) -> anyhow::Result<Vec<Topic>> {
        read_jsonl_closed(&shards::topics_path(&self.layout, scope))
    }
    pub(crate) fn closed_questions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Question>> {
        read_jsonl_closed(&shards::questions_path(&self.layout, scope))
    }
    pub(crate) fn closed_resolutions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Resolution>> {
        read_jsonl_closed(&shards::resolutions_path(&self.layout, scope))
    }
    pub(crate) fn closed_rules(&self, scope: &ScopeId) -> anyhow::Result<Vec<Rule>> {
        read_jsonl_closed(&shards::rules_path(&self.layout, scope))
    }
    pub(crate) fn closed_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        read_jsonl_closed(&shards::verification_bindings_path(&self.layout, scope))
    }
    pub(crate) fn closed_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        read_jsonl_closed(&shards::implementation_bindings_path(&self.layout, scope))
    }
    pub(crate) fn closed_edges(&self, scope: &ScopeId) -> anyhow::Result<Vec<Edge>> {
        read_edge_shards(&self.layout, Some(scope))
    }
    pub fn list_threads(&self, scope: &ScopeId) -> anyhow::Result<Vec<Thread>> {
        read_jsonl(&shards::threads_path(&self.layout, scope))
    }
    pub fn list_messages(&self, scope: &ScopeId) -> anyhow::Result<Vec<Message>> {
        read_message_shards(&self.layout, scope)
    }
    pub fn list_contributions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Contribution>> {
        self.list_contributions_after_direct_read(scope, || Ok(()))
    }
    fn list_contributions_after_direct_read(
        &self,
        scope: &ScopeId,
        after_direct_read: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<Contribution>> {
        self.with_repository_publication(|| {
            let mut records = read_jsonl(&shards::contributions_path(&self.layout, scope))?;
            after_direct_read()?;
            for batch in self.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.contributions, |record| {
                    record.id.as_str()
                });
            }
            Ok(records)
        })
    }
    pub fn list_synthesis_packets(&self, scope: &ScopeId) -> anyhow::Result<Vec<SynthesisPacket>> {
        self.with_repository_publication(|| {
            let mut records = read_jsonl(&shards::synthesis_packets_path(&self.layout, scope))?;
            for batch in self.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.synthesis_packets, |record| {
                    record.id.as_str()
                });
            }
            Ok(records)
        })
    }
    pub fn list_proposal_cards(&self, scope: &ScopeId) -> anyhow::Result<Vec<ProposalCard>> {
        self.project_proposal_cards(scope, None, || Ok(()))
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
        self.with_repository_publication(|| {
            if let Some(disposition_actor_ids) = disposition_actor_ids {
                self.validate_ideation_scope_with_actor_ids(scope, disposition_actor_ids)?;
            } else {
                self.validate_ideation_scope(scope)?;
            }
            after_validation()?;
            let assertions = self.list_assertion_records(scope)?;
            let dispositions = self.list_dispositions(scope)?;
            Ok(self
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
        self.with_repository_publication(|| {
            let mut records = read_jsonl(&shards::proposal_cards_path(&self.layout, scope))?;
            for batch in self.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.proposals, |record| record.id.as_str());
            }
            Ok(records)
        })
    }
    pub fn list_dispositions(&self, scope: &ScopeId) -> anyhow::Result<Vec<DispositionRecord>> {
        self.with_repository_publication(|| {
            let mut records = read_jsonl(&shards::dispositions_path(&self.layout, scope))?;
            records.extend(read_legacy_dispositions(
                &shards::legacy_promotion_decisions_path(&self.layout, scope),
            )?);
            for batch in self.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.dispositions, |record| {
                    record.id.as_str()
                });
            }
            Ok(records)
        })
    }
    pub fn list_assertion_records(&self, scope: &ScopeId) -> anyhow::Result<Vec<AssertionRecord>> {
        self.with_repository_publication(|| {
            let mut records = read_jsonl(&shards::assertion_records_path(&self.layout, scope))?;
            for batch in self.list_ideation_landings(scope)? {
                overlay_records(&mut records, batch.assertions, |record| record.id.as_str());
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

#[cfg(test)]
mod tests;
