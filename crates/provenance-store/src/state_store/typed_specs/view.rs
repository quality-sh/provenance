//! The records a typed-spec decision reads.

use crate::dictionary_reference::ProjectDictionary;
use provenance_core::{
    Boundary, ImplementationBinding, Question, Requirement, Resolution, Rule, ScopeId, Source,
    Topic,
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
    pub boundaries: Vec<Boundary>,
    /// The project dictionary that statement diagnostics use.
    pub dictionary: Option<ProjectDictionary>,
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
            boundaries: self.list_boundaries(scope)?,
            dictionary: crate::dictionary_reference::load_project_dictionary(&self.layout),
        })
    }
}
