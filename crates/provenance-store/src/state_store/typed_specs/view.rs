//! The records a typed-spec decision reads.

use crate::dictionary_reference::ProjectDictionary;
use provenance_core::{
    CanonicalArtifactType, DispositionRecord, ImplementationBinding, NodeType, Question,
    Requirement, Resolution, Rule, ScopeId, Source, StableId, Topic,
};

use crate::shards;
use crate::state_store::StateStore;

/// The records of one scope that a typed-spec decision reads.
///
/// A caller that holds the records builds the view directly. The store reads
/// it with [`StateStore::typed_spec_view`].
#[derive(Debug, Clone, Default)]
pub struct TypedSpecView {
    pub sources: Vec<Source>,
    pub requirements: Vec<Requirement>,
    pub rules: Vec<Rule>,
    pub implementation_bindings: Vec<ImplementationBinding>,
    pub resolutions: Vec<Resolution>,
    pub topics: Vec<Topic>,
    pub questions: Vec<Question>,
    /// The project dictionary that statement diagnostics use.
    pub dictionary: Option<ProjectDictionary>,
    /// The review history that keeps a record from a typed-spec deletion.
    pub review: ReviewHistory,
}

/// The review records of one scope that keep a record from a typed-spec
/// deletion.
#[derive(Debug, Clone, Default)]
pub struct ReviewHistory {
    /// The kind and id of each record that has a review submission, in any
    /// state.
    pub submitted: Vec<(NodeType, StableId)>,
    /// Each Disposition of the scope, in stored order.
    pub dispositions: Vec<DispositionRecord>,
}

impl ReviewHistory {
    /// Whether the record has a review submission, in any state.
    pub fn has_submission(&self, kind: NodeType, id: &StableId) -> bool {
        self.submitted
            .iter()
            .any(|(record_kind, record_id)| *record_kind == kind && record_id == id)
    }
}

impl TypedSpecView {
    /// The records of the view by kind.
    pub(super) fn records(&self) -> ScopeRecords<'_> {
        ScopeRecords {
            sources: &self.sources,
            requirements: &self.requirements,
            rules: &self.rules,
            resolutions: &self.resolutions,
            topics: &self.topics,
            questions: &self.questions,
        }
    }
}

/// The graph records of one scope, by kind.
pub(super) struct ScopeRecords<'a> {
    pub(super) sources: &'a [Source],
    pub(super) requirements: &'a [Requirement],
    pub(super) rules: &'a [Rule],
    pub(super) resolutions: &'a [Resolution],
    pub(super) topics: &'a [Topic],
    pub(super) questions: &'a [Question],
}

impl ScopeRecords<'_> {
    /// Whether a record has the kind and the id, and the scope when it is
    /// given.
    pub(super) fn contain(&self, kind: NodeType, scope: Option<&ScopeId>, id: &StableId) -> bool {
        match kind {
            NodeType::Source => holds(self.sources, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Requirement => holds(self.requirements, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Rule => holds(self.rules, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Resolution => holds(self.resolutions, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Topic => holds(self.topics, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Question => holds(self.questions, |r| (&r.scope_id, &r.id), scope, id),
        }
    }
}

fn holds<T>(
    records: &[T],
    identity: fn(&T) -> (&ScopeId, &StableId),
    scope: Option<&ScopeId>,
    id: &StableId,
) -> bool {
    records
        .iter()
        .map(identity)
        .any(|(record_scope, record_id)| {
            record_id == id && scope.is_none_or(|scope| scope == record_scope)
        })
}

/// The kind of record that a canonical artifact names.
pub(super) const fn node_type(kind: CanonicalArtifactType) -> NodeType {
    match kind {
        CanonicalArtifactType::Source => NodeType::Source,
        CanonicalArtifactType::Requirement => NodeType::Requirement,
        CanonicalArtifactType::Resolution => NodeType::Resolution,
        CanonicalArtifactType::Rule => NodeType::Rule,
        CanonicalArtifactType::Topic => NodeType::Topic,
        CanonicalArtifactType::Question => NodeType::Question,
    }
}

impl StateStore {
    /// Reads the view under the section the caller holds.
    pub fn typed_spec_view(&self, scope: &ScopeId) -> anyhow::Result<TypedSpecView> {
        let view = TypedSpecView {
            sources: self.list_sources(scope)?,
            requirements: self.list_requirements(scope)?,
            rules: self.list_rules(scope)?,
            implementation_bindings: self.list_implementation_bindings(scope)?,
            resolutions: self.list_resolutions(scope)?,
            topics: self.list_topics(scope)?,
            questions: self.list_questions(scope)?,
            dictionary: self.project_dictionary().as_deref().cloned(),
            review: self.review_history(scope)?,
        };
        self.ensure_dispositions_name_records(scope, &view)?;
        Ok(view)
    }

    /// Refuses, at its line, a Disposition that names a record that the
    /// scope does not have.
    fn ensure_dispositions_name_records(
        &self,
        scope: &ScopeId,
        view: &TypedSpecView,
    ) -> anyhow::Result<()> {
        let records = view.records();
        for disposition in &view.review.dispositions {
            let Some(artifact) = &disposition.canonical_artifact else {
                continue;
            };
            let kind = node_type(artifact.artifact_type);
            if records.contain(kind, Some(scope), &artifact.artifact_id) {
                continue;
            }
            let message = format!(
                "disposition {} names {} {}, which the scope does not have",
                disposition.id.as_str(),
                kind.as_str(),
                artifact.artifact_id.as_str()
            );
            return Err(crate::state_store::readers::record_fault(
                self,
                &shards::dispositions_path(&self.layout, scope),
                disposition.id.as_str(),
                message,
            ));
        }
        Ok(())
    }
}
