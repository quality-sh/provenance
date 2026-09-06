use super::StateStore;
use provenance_core::model::relations::kind_word;
use provenance_core::{
    CanonicalArtifact, CanonicalArtifactType, IdeationTarget, NodeType, ScopeId, StableId,
};
use provenance_macros::rule;
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq, Hash)]
struct RecordKey {
    kind: &'static str,
    id: String,
}

/// Every record of the eight graph kinds in one scope, keyed by the kind's
/// serde word and the record id. Dispositions and ideation writers ask it
/// whether the record they name exists.
pub(super) struct CanonicalArtifactIndex {
    scope_id: ScopeId,
    entries: HashSet<RecordKey>,
}

impl CanonicalArtifactIndex {
    fn load(store: &StateStore, scope_id: &ScopeId) -> anyhow::Result<Self> {
        let mut entries = HashSet::new();
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Source,
            store.list_sources(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Requirement,
            store.list_requirements(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Resolution,
            store.list_resolutions(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Rule,
            store.list_rules(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Topic,
            store.list_topics(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Question,
            store.list_questions(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Domain,
            store.list_domains(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Boundary,
            store.list_boundaries(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        Ok(Self {
            scope_id: scope_id.clone(),
            entries,
        })
    }

    pub(super) fn ensure_exists(&self, artifact: Option<&CanonicalArtifact>) -> anyhow::Result<()> {
        let Some(artifact) = artifact else {
            return Ok(());
        };
        let kind = match artifact.artifact_type {
            CanonicalArtifactType::Source => NodeType::Source,
            CanonicalArtifactType::Requirement => NodeType::Requirement,
            CanonicalArtifactType::Resolution => NodeType::Resolution,
            CanonicalArtifactType::Rule => NodeType::Rule,
        };
        anyhow::ensure!(
            self.entries.contains(&key(kind, &artifact.artifact_id)),
            "canonical artifact does not exist in scope {} with kind {:?}: {}",
            self.scope_id.as_str(),
            artifact.artifact_type,
            artifact.artifact_id.as_str()
        );
        Ok(())
    }

    /// A new contribution or synthesis packet names a record that exists in
    /// its scope, of any of the eight kinds. Only the ideation writers call
    /// this, the direct ones and the landed batch; a read never does.
    #[rule("rule_new_ideation_target_names_a_record")]
    pub(super) fn ensure_target_exists(&self, target: &IdeationTarget) -> anyhow::Result<()> {
        let kind = NodeType::from(target.artifact_type);
        anyhow::ensure!(
            self.entries.contains(&key(kind, &target.artifact_id)),
            "target points at missing {} {} in scope {}",
            kind_word(kind),
            target.artifact_id.as_str(),
            self.scope_id.as_str()
        );
        Ok(())
    }
}

impl StateStore {
    pub(super) fn canonical_artifact_index(
        &self,
        scope_id: &ScopeId,
    ) -> anyhow::Result<CanonicalArtifactIndex> {
        CanonicalArtifactIndex::load(self, scope_id)
    }
}

fn key(kind: NodeType, id: &StableId) -> RecordKey {
    RecordKey {
        kind: kind_word(kind),
        id: id.as_str().to_owned(),
    }
}

fn extend_scoped<T>(
    entries: &mut HashSet<RecordKey>,
    scope_id: &ScopeId,
    kind: NodeType,
    records: Vec<T>,
    fields: impl Fn(T) -> (ScopeId, StableId),
) {
    entries.extend(records.into_iter().filter_map(|record| {
        let (embedded_scope_id, id) = fields(record);
        (embedded_scope_id == *scope_id).then(|| key(kind, &id))
    }));
}
