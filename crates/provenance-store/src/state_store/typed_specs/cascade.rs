//! Delete dependants with required parents and remove optional references.
use std::collections::{BTreeMap, BTreeSet};

use provenance_core::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, CanonicalArtifactType, NodeType, Question,
    Requirement, Resolution, Rule, ScopeId, Source, StableId, Topic,
};

use super::super::{
    CascadedResource, ReconcileState, ReconciledResource, StateStore, TypedFieldChange,
    TypedResourceKind,
};
use crate::shards;

pub(super) struct Cascade {
    pub(super) rules: BTreeSet<String>,
    resolutions: Vec<Resolution>,
    topics: Vec<Topic>,
    questions: Vec<Question>,
    boundaries: Vec<Boundary>,
    changes: Vec<CascadedResource>,
}

struct ShapingRecords {
    topics: Vec<Topic>,
    questions: Vec<Question>,
    boundaries: Vec<Boundary>,
    changes: Vec<CascadedResource>,
}

impl Cascade {
    pub(super) fn prepare(
        store: &StateStore,
        scope: &ScopeId,
        resources: &[ReconciledResource],
        sources: &mut [Source],
        requirements: &mut [Requirement],
        rules: &mut Vec<Rule>,
    ) -> anyhow::Result<Self> {
        let deleted = |kind| {
            resources
                .iter()
                .filter(|r| r.kind == kind && r.state == ReconcileState::Deleted)
                .map(|r| r.id.as_str().to_owned())
                .collect::<BTreeSet<_>>()
        };
        let source_ids = deleted(TypedResourceKind::Source);
        let requirement_ids = deleted(TypedResourceKind::Requirement);
        let mut rule_ids = deleted(TypedResourceKind::Rule);
        let before_sources = sources.to_vec();
        let before_requirements = requirements.to_vec();
        let before_rules = rules.clone();

        let mut resolutions = store.list_resolutions(scope)?;
        let before_resolutions = resolutions.clone();
        let mut resolution_ids = BTreeSet::new();
        resolutions.retain_mut(|record| {
            let changed = remove_ids(&mut record.requirement_ids, &requirement_ids);
            let remove = changed && record.requirement_ids.is_empty();
            if remove {
                resolution_ids.insert(record.id.as_str().to_owned());
            }
            !remove
        });
        for source in &mut *sources {
            remove_ids(&mut source.supersedes, &source_ids);
        }
        for requirement in &mut *requirements {
            requirement
                .source_refs
                .retain(|r| !source_ids.contains(r.source_id.as_str()));
            clear_id(&mut requirement.refines, &requirement_ids);
            remove_ids(&mut requirement.depends_on, &requirement_ids);
            remove_ids(&mut requirement.supersedes, &requirement_ids);
            clear_id(&mut requirement.spawned_by, &resolution_ids);
        }
        for resolution in &mut resolutions {
            remove_ids(&mut resolution.supersedes, &resolution_ids);
        }
        rules.retain_mut(|record| {
            let changed = remove_ids(&mut record.requirement_ids, &requirement_ids);
            remove_ids(&mut record.resolution_ids, &resolution_ids);
            let remove = changed && record.requirement_ids.is_empty();
            if remove {
                rule_ids.insert(record.id.as_str().to_owned());
            }
            !remove
        });

        let shaping = prepare_shaping(
            store,
            scope,
            &source_ids,
            &requirement_ids,
            &resolution_ids,
            &rule_ids,
        )?;

        let mut changes =
            describe_replacement(NodeType::Source, &before_sources, sources, |r| &r.id);
        changes.extend(describe_replacement(
            NodeType::Requirement,
            &before_requirements,
            requirements,
            |r| &r.id,
        ));
        changes.extend(describe_replacement(
            NodeType::Resolution,
            &before_resolutions,
            &resolutions,
            |r| &r.id,
        ));
        changes.extend(describe_replacement(
            NodeType::Rule,
            &before_rules,
            rules,
            |r| &r.id,
        ));
        changes.extend(shaping.changes);
        Ok(Self {
            rules: rule_ids,
            resolutions,
            topics: shaping.topics,
            questions: shaping.questions,
            boundaries: shaping.boundaries,
            changes,
        })
    }

    pub(super) fn report(&self, resources: &mut [ReconciledResource]) -> Vec<CascadedResource> {
        let mut cascade = Vec::new();
        for change in &self.changes {
            let Some(kind) = typed_kind(change.kind) else {
                cascade.push(change.clone());
                continue;
            };
            let Some(resource) = resources
                .iter_mut()
                .find(|resource| resource.kind == kind && resource.id == change.id)
            else {
                cascade.push(change.clone());
                continue;
            };
            if resource.state == ReconcileState::Unchanged {
                resource.state = change.state;
            }
            merge_changes(&mut resource.changes, &change.changes);
        }
        cascade
    }

    pub(super) fn ensure_dispositions_survive(
        &self,
        store: &StateStore,
        scope: &ScopeId,
        sources: &[Source],
        requirements: &[Requirement],
        rules: &[Rule],
    ) -> anyhow::Result<()> {
        for disposition in store.list_dispositions(scope)? {
            let Some(artifact) = &disposition.canonical_artifact else {
                continue;
            };
            let survives = match artifact.artifact_type {
                CanonicalArtifactType::Source => {
                    contains_id(sources, &artifact.artifact_id, |r| &r.id)
                }
                CanonicalArtifactType::Requirement => {
                    contains_id(requirements, &artifact.artifact_id, |r| &r.id)
                }
                CanonicalArtifactType::Resolution => {
                    contains_id(&self.resolutions, &artifact.artifact_id, |r| &r.id)
                }
                CanonicalArtifactType::Rule => contains_id(rules, &artifact.artifact_id, |r| &r.id),
            };
            anyhow::ensure!(
                survives,
                "cannot delete canonical {} {} because disposition {} names it",
                canonical_kind(artifact.artifact_type),
                artifact.artifact_id.as_str(),
                disposition.id.as_str()
            );
        }
        Ok(())
    }

    pub(super) fn publish(self, store: &StateStore, scope: &ScopeId) -> anyhow::Result<()> {
        store.replace_graph_records(
            &shards::resolutions_path(&store.layout, scope),
            self.resolutions,
        )?;
        super::replace_records(
            store,
            &shards::topics_path(&store.layout, scope),
            self.topics,
        )?;
        super::replace_records(
            store,
            &shards::questions_path(&store.layout, scope),
            self.questions,
        )?;
        super::replace_records(
            store,
            &shards::boundaries_path(&store.layout, scope),
            self.boundaries,
        )?;
        store.mutate_jsonl_records(
            &shards::verification_bindings_path(&store.layout, scope),
            |records: &mut Vec<provenance_core::VerificationBinding>| {
                records.retain(|r| !self.rules.contains(r.rule_id.as_str()));
                Ok(())
            },
        )
    }
}

fn prepare_shaping(
    store: &StateStore,
    scope: &ScopeId,
    source_ids: &BTreeSet<String>,
    requirement_ids: &BTreeSet<String>,
    resolution_ids: &BTreeSet<String>,
    rule_ids: &BTreeSet<String>,
) -> anyhow::Result<ShapingRecords> {
    let mut topics = store.list_topics(scope)?;
    let before_topics = topics.clone();
    let mut topic_ids = BTreeSet::new();
    topics.retain_mut(|record| {
        let remove = requirement_ids.contains(record.requirement_id.as_str());
        if remove {
            topic_ids.insert(record.id.as_str().to_owned());
        } else {
            retain_links(
                &mut record.links,
                source_ids,
                requirement_ids,
                resolution_ids,
                rule_ids,
            );
        }
        !remove
    });
    let mut questions = store.list_questions(scope)?;
    let before_questions = questions.clone();
    questions.retain_mut(|record| {
        let remove = requirement_ids.contains(record.requirement_id.as_str())
            || topic_ids.contains(record.topic_id.as_str());
        if !remove {
            clear_id(&mut record.contradicts, requirement_ids);
            clear_id(&mut record.resolution_id, resolution_ids);
            retain_links(
                &mut record.links,
                source_ids,
                requirement_ids,
                resolution_ids,
                rule_ids,
            );
        }
        !remove
    });
    let mut boundaries = store.list_boundaries(scope)?;
    let before_boundaries = boundaries.clone();
    boundaries.retain(|r| !requirement_ids.contains(r.requirement_id.as_str()));
    for boundary in &mut boundaries {
        if boundary
            .source_ref
            .as_ref()
            .is_some_and(|r| source_ids.contains(r.source_id.as_str()))
        {
            boundary.source_ref = None;
        }
    }
    let mut changes = describe_replacement(NodeType::Topic, &before_topics, &topics, |r| &r.id);
    changes.extend(describe_replacement(
        NodeType::Question,
        &before_questions,
        &questions,
        |r| &r.id,
    ));
    changes.extend(describe_replacement(
        NodeType::Boundary,
        &before_boundaries,
        &boundaries,
        |r| &r.id,
    ));
    Ok(ShapingRecords {
        topics,
        questions,
        boundaries,
        changes,
    })
}

fn describe_replacement<'a, T: serde::Serialize>(
    kind: NodeType,
    before: &'a [T],
    after: &'a [T],
    id: impl Fn(&'a T) -> &'a StableId,
) -> Vec<CascadedResource> {
    let after_by_id = after
        .iter()
        .map(|record| (id(record).as_str(), record))
        .collect::<BTreeMap<_, _>>();
    before
        .iter()
        .filter_map(|record| {
            let record_id = id(record);
            let Some(replacement) = after_by_id.get(record_id.as_str()) else {
                return Some(CascadedResource {
                    kind,
                    id: record_id.clone(),
                    state: ReconcileState::Deleted,
                    changes: Vec::new(),
                });
            };
            let changes = field_changes(record, replacement);
            (!changes.is_empty()).then(|| CascadedResource {
                kind,
                id: record_id.clone(),
                state: ReconcileState::Updated,
                changes,
            })
        })
        .collect()
}

fn field_changes<T: serde::Serialize>(before: &T, after: &T) -> Vec<TypedFieldChange> {
    let before = serde_json::to_value(before).expect("canonical record serializes");
    let after = serde_json::to_value(after).expect("canonical record serializes");
    let before = before.as_object().expect("canonical record is an object");
    let after = after.as_object().expect("canonical record is an object");
    before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|field| {
            let old = before
                .get(field)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let new = after.get(field).cloned().unwrap_or(serde_json::Value::Null);
            (old != new).then(|| TypedFieldChange {
                field: field.clone(),
                before: old,
                after: new,
            })
        })
        .collect()
}

fn merge_changes(current: &mut Vec<TypedFieldChange>, cascade: &[TypedFieldChange]) {
    for change in cascade {
        if let Some(existing) = current
            .iter_mut()
            .find(|existing| existing.field == change.field)
        {
            existing.after.clone_from(&change.after);
        } else {
            current.push(change.clone());
        }
    }
}

const fn typed_kind(kind: NodeType) -> Option<TypedResourceKind> {
    match kind {
        NodeType::Source => Some(TypedResourceKind::Source),
        NodeType::Requirement => Some(TypedResourceKind::Requirement),
        NodeType::Rule => Some(TypedResourceKind::Rule),
        NodeType::Resolution
        | NodeType::Topic
        | NodeType::Question
        | NodeType::Domain
        | NodeType::Boundary => None,
    }
}

fn retain_links(
    links: &mut Vec<ArtifactLink>,
    sources: &BTreeSet<String>,
    requirements: &BTreeSet<String>,
    resolutions: &BTreeSet<String>,
    rules: &BTreeSet<String>,
) {
    links.retain(|link| {
        let deleted = match link.target_type {
            ArtifactLinkTargetType::Source => sources,
            ArtifactLinkTargetType::Requirement => requirements,
            ArtifactLinkTargetType::Resolution => resolutions,
            ArtifactLinkTargetType::Rule => rules,
        };
        !deleted.contains(link.target_id.as_str())
    });
}

fn contains_id<'a, T>(
    records: &'a [T],
    wanted: &StableId,
    id: impl Fn(&'a T) -> &'a StableId,
) -> bool {
    records.iter().any(|record| id(record) == wanted)
}

const fn canonical_kind(kind: CanonicalArtifactType) -> &'static str {
    match kind {
        CanonicalArtifactType::Source => "source",
        CanonicalArtifactType::Requirement => "requirement",
        CanonicalArtifactType::Resolution => "resolution",
        CanonicalArtifactType::Rule => "rule",
    }
}

fn remove_ids(ids: &mut Vec<StableId>, deleted: &BTreeSet<String>) -> bool {
    let before = ids.len();
    ids.retain(|id| !deleted.contains(id.as_str()));
    before != ids.len()
}

fn clear_id(id: &mut Option<StableId>, deleted: &BTreeSet<String>) {
    if id.as_ref().is_some_and(|id| deleted.contains(id.as_str())) {
        *id = None;
    }
}
