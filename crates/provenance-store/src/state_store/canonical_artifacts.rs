use super::message_ids::ShardLine;
use super::StateStore;
use provenance_core::model::relations::kind_word;
use provenance_core::{
    ensure_record_id_assignable, CanonicalArtifact, DispositionRecord, IdeationTarget, NodeType,
    Scope, ScopeId, StableId,
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
            store,
            &crate::shards::sources_path(&store.layout, scope_id),
            store.list_sources(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Requirement,
            store,
            &crate::shards::requirements_path(&store.layout, scope_id),
            store.list_requirements(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Resolution,
            store,
            &crate::shards::resolutions_path(&store.layout, scope_id),
            store.list_resolutions(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Rule,
            store,
            &crate::shards::rules_path(&store.layout, scope_id),
            store.list_rules(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Topic,
            store,
            &crate::shards::topics_path(&store.layout, scope_id),
            store.list_topics(scope_id)?,
            |record| (record.scope_id, record.id),
        )?;
        extend_scoped(
            &mut entries,
            scope_id,
            NodeType::Question,
            store,
            &crate::shards::questions_path(&store.layout, scope_id),
            store.list_questions(scope_id)?,
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
                    store,
                    &crate::shards::$path(&store.layout, scope_id),
                    super::readers::read_jsonl_held::<$ty>(
                        store,
                        &crate::shards::$path(&store.layout, scope_id),
                    )?,
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
        Ok(Self {
            scope_id: scope_id.clone(),
            entries,
        })
    }

    pub(super) fn ensure_exists(&self, artifact: Option<&CanonicalArtifact>) -> anyhow::Result<()> {
        let Some(artifact) = artifact else {
            return Ok(());
        };
        crate::write_error::ensure!(
            MissingReference,
            self.contains(artifact.artifact_type.into(), &artifact.artifact_id),
            "canonical artifact does not exist in scope {} with kind {:?}: {}",
            self.scope_id.as_str(),
            artifact.artifact_type,
            artifact.artifact_id.as_str()
        );
        Ok(())
    }

    /// Whether the scope has the record of the kind and the id.
    pub(super) fn contains(&self, kind: NodeType, id: &StableId) -> bool {
        self.entries.contains(&key(kind, id))
    }

    /// A new contribution or synthesis packet names a record that exists in
    /// its scope, of any of the eight kinds. Only the ideation writers call
    /// this, the direct ones and the landed batch; a read never does.
    #[rule("rule_new_ideation_target_names_a_record")]
    pub(super) fn ensure_target_exists(&self, target: &IdeationTarget) -> anyhow::Result<()> {
        let kind = NodeType::from(target.artifact_type);
        crate::write_error::ensure!(
            MissingReference,
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
    /// Refuses a stored Disposition of `dispositions` that names a record
    /// that the scope does not have. `has_record` tells whether the scope
    /// has the record of a kind and an id.
    pub(super) fn ensure_dispositions_name_records(
        &self,
        scope: &ScopeId,
        dispositions: &[DispositionRecord],
        has_record: impl Fn(NodeType, &StableId) -> bool,
    ) -> anyhow::Result<()> {
        for disposition in dispositions {
            let Some(artifact) = &disposition.canonical_artifact else {
                continue;
            };
            let kind = NodeType::from(artifact.artifact_type);
            if has_record(kind, &artifact.artifact_id) {
                continue;
            }
            let message = format!(
                "disposition {} names {} {}, which the scope does not have",
                disposition.id.as_str(),
                kind.as_str(),
                artifact.artifact_id.as_str()
            );
            return Err(self.disposition_fault(scope, &disposition.id, message));
        }
        Ok(())
    }

    /// The refusal `invalid_state_file` at the line that stores the
    /// Disposition `id`: the line of the last landing that holds it, as a
    /// later landing replaces an earlier one, else its line in the
    /// Disposition file.
    fn disposition_fault(&self, scope: &ScopeId, id: &StableId, message: String) -> anyhow::Error {
        // An error of this read is not given: it would replace the fault.
        let landings = self
            .with_repository_read(|held| held.list_ideation_landings(scope))
            .unwrap_or_default();
        let landing = landings.iter().rposition(|batch| {
            batch.dispositions.iter().any(|disposition| disposition.id == *id)
        });
        match landing {
            Some(index) => crate::refusal::state_fault(
                &ShardLine::new(
                    &crate::shards::ideation_landings_path(&self.layout, scope),
                    index + 1,
                ),
                message,
            ),
            None => super::readers::record_fault(
                self,
                &crate::shards::dispositions_path(&self.layout, scope),
                id.as_str(),
                message,
            ),
        }
    }

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
        ensure_record_id_assignable(id.as_str())?;
        self.ensure_canonical_replacement_ids_unique(scope_id, std::iter::once((None, id)), &[])
    }

    pub(super) fn ensure_canonical_replacement_ids_unique<'a>(
        &self,
        scope_id: &ScopeId,
        replacements: impl IntoIterator<Item = (Option<NodeType>, &'a StableId)>,
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
        replacements: impl IntoIterator<Item = (NodeType, &'a StableId)>,
        replaced_kinds: &[NodeType],
    ) -> anyhow::Result<()> {
        let manifest = super::manifest_from_bytes(&self.read_state(&self.layout.manifest_path())?)?;
        ensure_replacement_ids_unique(
            Some(scope_id),
            replacements.into_iter().map(|(kind, id)| (Some(kind), id)),
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
    replacements: impl IntoIterator<Item = (Option<NodeType>, &'a StableId)>,
    replaced_kinds: &[NodeType],
    scopes: impl IntoIterator<Item = anyhow::Result<(ScopeId, CanonicalArtifactIndex)>>,
) -> anyhow::Result<()> {
    let replaced_kinds = replaced_kinds
        .iter()
        .map(|kind| kind_word(*kind))
        .collect::<HashSet<_>>();
    let mut ids = HashSet::new();
    let mut previous_keys = HashSet::new();
    for scope in scopes {
        let (current_scope, index) = scope?;
        for entry in index.entries {
            if scope_id.is_some_and(|scope_id| current_scope == *scope_id) {
                previous_keys.insert(RecordKey {
                    kind: entry.kind,
                    id: entry.id.clone(),
                });
            }
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
    for (kind, id) in replacements {
        if !kind.is_some_and(|kind| previous_keys.contains(&key(kind, id))) {
            ensure_record_id_assignable(id.as_str())?;
        }
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
    store: &StateStore,
    path: &camino::Utf8Path,
    records: Vec<T>,
    fields: impl Fn(T) -> (ScopeId, StableId),
) -> anyhow::Result<()> {
    for record in records {
        let (embedded_scope_id, id) = fields(record);
        if embedded_scope_id != *scope_id {
            continue;
        }
        if !entries.insert(key(kind, &id)) {
            return Err(super::readers::record_fault(
                store,
                path,
                id.as_str(),
                format!(
                    "{} {} appears more than once in scope {}",
                    kind_word(kind),
                    id.as_str(),
                    scope_id.as_str()
                ),
            ));
        }
    }
    Ok(())
}
