//! The records a typed-spec decision reads.

use crate::dictionary_reference::ProjectDictionary;
use provenance_core::{
    DispositionRecord, ImplementationBinding, NodeType, Question, Requirement, Resolution, Rule,
    ScopeId, Source, StableId, Topic,
};

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

impl StateStore {
    /// Reads the view under the section the caller holds.
    pub fn typed_spec_view(&self, scope: &ScopeId) -> anyhow::Result<TypedSpecView> {
        Ok(TypedSpecView {
            sources: self.list_sources(scope)?,
            requirements: self.list_requirements(scope)?,
            rules: self.list_rules(scope)?,
            implementation_bindings: self.list_implementation_bindings(scope)?,
            resolutions: self.list_resolutions(scope)?,
            topics: self.list_topics(scope)?,
            questions: self.list_questions(scope)?,
            dictionary: self.project_dictionary().as_deref().cloned(),
            review: self.review_history(scope)?,
        })
    }
}
