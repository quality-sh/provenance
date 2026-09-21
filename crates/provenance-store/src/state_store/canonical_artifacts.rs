use super::StateStore;
use provenance_core::model::relations::kind_word;
use provenance_core::{
    CanonicalArtifact, CanonicalArtifactType, IdeationTarget, NodeType, Scope, ScopeId, StableId,
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
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Requirement,
            store.list_requirements(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Resolution,
            store.list_resolutions(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Rule,
            store.list_rules(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Topic,
            store.list_topics(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Question,
            store.list_questions(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Domain,
            store.list_domains(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Boundary,
            store.list_boundaries(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        Ok(Self {
            scope_id: scope_id.clone(),
            entries,
        })
    }

    fn load_unlocked(store: &StateStore, scope_id: &ScopeId) -> anyhow::Result<Self> {
        let mut entries = HashSet::new();
        macro_rules! extend {
            ($kind:ident, $path:ident, $ty:ty) => {
                extend_scoped(
                    &mut entries,
                    scope_id,
                    NodeType::$kind,
                    super::readers::read_jsonl_unlocked::<$ty>(&crate::shards::$path(
                        &store.layout,
                        scope_id,
                    ))?,
                    |record| (record.scope_id, record.id),
                )?;
            };
        }
        extend!(Source, sources_path, provenance_core::Source);
        extend!(Requirement, requirements_path, provenance_core::Requirement);
        extend!(Resolution, resolutions_path, provenance_core::Resolution);
        extend!(Rule, rules_path, provenance_core::Rule);
        extend!(Topic, topics_path, provenance_core::Topic);
        extend!(Question, questions_path, provenance_core::Question);
        extend!(Domain, domains_path, provenance_core::Domain);
        extend!(Boundary, boundaries_path, provenance_core::Boundary);
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

    /// Refuses a canonical ID that any scope in this repository already uses.
    #[rule("rule_porcelain_id_unique_in_repository")]
    pub(super) fn ensure_canonical_id_available(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<()> {
        self.ensure_canonical_replacement_ids_unique(scope_id, std::iter::once(id), &[])
    }

    pub(super) fn ensure_canonical_replacement_ids_unique<'a>(
        &self,
        scope_id: &ScopeId,
        replacements: impl IntoIterator<Item = &'a StableId>,
        replaced_kinds: &[NodeType],
    ) -> anyhow::Result<()> {
        let manifest = self.manifest()?;
        ensure_replacement_ids_unique(
            Some(scope_id),
            replacements,
            replaced_kinds,
            manifest.scopes.into_iter().map(|scope| {
                CanonicalArtifactIndex::load(self, &scope.id).map(|index| (scope.id, index))
            }),
        )
    }

    /// Validate the repository-wide identity invariant for stored records.
    pub fn validate_canonical_ids_unique(&self, scopes: &[Scope]) -> anyhow::Result<()> {
        ensure_replacement_ids_unique(
            None,
            std::iter::empty(),
            &[],
            scopes.iter().map(|scope| {
                CanonicalArtifactIndex::load(self, &scope.id).map(|index| (scope.id.clone(), index))
            }),
        )
    }

    pub(super) fn ensure_import_ids_unique<'a>(
        &self,
        scope_id: &ScopeId,
        replacements: impl IntoIterator<Item = &'a StableId>,
        replaced_kinds: &[NodeType],
    ) -> anyhow::Result<()> {
        let manifest = super::manifest_from_bytes(&std::fs::read(self.layout.manifest_path())?)?;
        ensure_replacement_ids_unique(
            Some(scope_id),
            replacements,
            replaced_kinds,
            manifest.scopes.into_iter().map(|scope| {
                CanonicalArtifactIndex::load_unlocked(self, &scope.id)
                    .map(|index| (scope.id, index))
            }),
        )
    }
}

fn ensure_replacement_ids_unique<'a>(
    scope_id: Option<&ScopeId>,
    replacements: impl IntoIterator<Item = &'a StableId>,
    replaced_kinds: &[NodeType],
    scopes: impl IntoIterator<Item = anyhow::Result<(ScopeId, CanonicalArtifactIndex)>>,
) -> anyhow::Result<()> {
    let replaced_kinds = replaced_kinds
        .iter()
        .map(|kind| kind_word(*kind))
        .collect::<HashSet<_>>();
    let mut ids = HashSet::new();
    for scope in scopes {
        let (current_scope, index) = scope?;
        for entry in index.entries {
            if scope_id.is_some_and(|scope_id| current_scope == *scope_id)
                && replaced_kinds.contains(entry.kind)
            {
                continue;
            }
            crate::write_error::ensure!(
                AlreadyExists,
                ids.insert(entry.id.clone()),
                "record ID {} appears more than once in this repository",
                entry.id
            );
        }
    }
    for id in replacements {
        crate::write_error::ensure!(
            AlreadyExists,
            ids.insert(id.as_str().to_owned()),
            "record ID already exists in this repository"
        );
    }
    Ok(())
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
) -> anyhow::Result<()> {
    for record in records {
        let (embedded_scope_id, id) = fields(record);
        if embedded_scope_id != *scope_id {
            continue;
        }
        anyhow::ensure!(
            entries.insert(key(kind, &id)),
            "{} {} appears more than once in scope {}",
            kind_word(kind),
            id.as_str(),
            scope_id.as_str()
        );
    }
    Ok(())
}
