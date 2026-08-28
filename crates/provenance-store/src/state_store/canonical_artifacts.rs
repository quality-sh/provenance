use super::StateStore;
use provenance_core::{CanonicalArtifact, CanonicalArtifactType, ScopeId, StableId};
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq, Hash)]
struct CanonicalArtifactKey {
    artifact_type: &'static str,
    artifact_id: String,
}

pub(super) struct CanonicalArtifactIndex {
    scope_id: ScopeId,
    entries: HashSet<CanonicalArtifactKey>,
}

impl CanonicalArtifactIndex {
    fn load(store: &StateStore, scope_id: &ScopeId) -> anyhow::Result<Self> {
        let mut entries = HashSet::new();
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Source,
            store.list_sources(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Requirement,
            store.list_requirements(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Resolution,
            store.list_resolutions(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Rule,
            store.list_rules(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Topic,
            store.list_topics(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Question,
            store.list_questions(scope_id)?,
            |record| (record.scope_id, record.id),
        );
        extend_scoped(
            &mut entries,
            scope_id,
            CanonicalArtifactType::Domain,
            store.list_domains(scope_id)?,
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
        anyhow::ensure!(
            self.entries
                .contains(&key(artifact.artifact_type, &artifact.artifact_id)),
            "canonical artifact does not exist in scope {} with kind {:?}: {}",
            self.scope_id.as_str(),
            artifact.artifact_type,
            artifact.artifact_id.as_str()
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

fn key(artifact_type: CanonicalArtifactType, artifact_id: &StableId) -> CanonicalArtifactKey {
    let artifact_type = match artifact_type {
        CanonicalArtifactType::Source => "source",
        CanonicalArtifactType::Requirement => "requirement",
        CanonicalArtifactType::Resolution => "resolution",
        CanonicalArtifactType::Rule => "rule",
        CanonicalArtifactType::Topic => "topic",
        CanonicalArtifactType::Question => "question",
        CanonicalArtifactType::Domain => "domain",
    };
    CanonicalArtifactKey {
        artifact_type,
        artifact_id: artifact_id.as_str().to_owned(),
    }
}

impl CanonicalArtifactIndex {}

fn extend_scoped<T>(
    entries: &mut HashSet<CanonicalArtifactKey>,
    scope_id: &ScopeId,
    artifact_type: CanonicalArtifactType,
    records: Vec<T>,
    fields: impl Fn(T) -> (ScopeId, StableId),
) {
    entries.extend(records.into_iter().filter_map(|record| {
        let (embedded_scope_id, artifact_id) = fields(record);
        (embedded_scope_id == *scope_id).then(|| key(artifact_type, &artifact_id))
    }));
}

#[cfg(test)]
mod superset_index {
    use super::*;

    /// The disposal extends the existence index over the whole superset
    /// vocabulary, so a canonical artifact naming topic, question, or
    /// domain resolves like the four canonical kinds do.
    #[test]
    fn superset_index_resolves_the_widened_vocabulary() {
        let directory = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = crate::layout::ProvenanceLayout::new(root);
        std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_string(&provenance_core::Manifest::default_with_scope(
                ScopeId::new("default").unwrap(),
                provenance_core::RepoPathPrefix::new("."),
            ))
            .unwrap(),
        )
        .unwrap();
        let scope = ScopeId::new("default").unwrap();
        let store = StateStore::new(layout);

        let entries = vec![
            ("topic", "topic_shaping"),
            ("question", "question_open"),
            ("domain", "domain_payroll"),
        ];
        let mut resolved = Vec::new();
        for (kind, id) in &entries {
            resolved.push(index_entry(kind, id, &scope, &store));
        }
        let index = store.canonical_artifact_index(&scope).unwrap();
        for (position, (kind, id)) in entries.iter().enumerate() {
            let word = match *kind {
                "topic" => CanonicalArtifactType::Topic,
                "question" => CanonicalArtifactType::Question,
                _ => CanonicalArtifactType::Domain,
            };
            let artifact = CanonicalArtifact {
                artifact_type: word,
                artifact_id: StableId::new(*id).unwrap(),
            };
            let exists = index.ensure_exists(Some(&artifact)).is_ok();
            assert_eq!(
                exists, resolved[position],
                "{kind} {id} must match the graph"
            );
        }
    }

    /// Writes one superset record and reports whether the graph holds it.
    fn index_entry(kind: &str, id: &str, scope: &ScopeId, store: &StateStore) -> bool {
        match kind {
            "topic" => store
                .create_topic(crate::state_store::CreateTopicInput {
                    scope_id: scope.clone(),
                    id: StableId::new(id).unwrap(),
                    requirement_id: StableId::new("req_anchor").unwrap(),
                    title: "T".into(),
                    status: provenance_core::TopicStatus::Open,
                    links: Vec::new(),
                })
                .is_ok(),
            "question" => false,
            _ => store
                .create_domain(crate::state_store::CreateDomainInput {
                    scope_id: scope.clone(),
                    id: StableId::new(id).unwrap(),
                    name: "D".into(),
                    description: None,
                    color: None,
                })
                .is_ok(),
        }
    }
}
