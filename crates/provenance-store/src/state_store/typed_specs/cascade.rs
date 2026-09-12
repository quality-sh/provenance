//! Delete dependants with required parents and remove optional references.
use std::collections::BTreeSet;

use provenance_core::{
    Boundary, Question, Requirement, Resolution, Rule, ScopeId, Source, StableId, Topic,
};

use super::super::{ReconcileState, ReconciledResource, StateStore, TypedResourceKind};
use crate::shards;

pub(super) struct Cascade {
    pub(super) rules: BTreeSet<String>,
    resolutions: Vec<Resolution>,
    topics: Vec<Topic>,
    questions: Vec<Question>,
    boundaries: Vec<Boundary>,
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
        let mut resolutions = store.list_resolutions(scope)?;
        let mut resolution_ids = BTreeSet::new();
        resolutions.retain_mut(|record| {
            let changed = remove_ids(&mut record.requirement_ids, &requirement_ids);
            let remove = changed && record.requirement_ids.is_empty();
            if remove {
                resolution_ids.insert(record.id.as_str().to_owned());
            }
            !remove
        });
        for source in sources {
            remove_ids(&mut source.supersedes, &source_ids);
        }
        for requirement in requirements {
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
        let mut topics = store.list_topics(scope)?;
        let mut topic_ids = BTreeSet::new();
        topics.retain(|record| {
            let remove = requirement_ids.contains(record.requirement_id.as_str());
            if remove {
                topic_ids.insert(record.id.as_str().to_owned());
            }
            !remove
        });
        let mut questions = store.list_questions(scope)?;
        questions.retain(|r| {
            !requirement_ids.contains(r.requirement_id.as_str())
                && !topic_ids.contains(r.topic_id.as_str())
        });
        let mut boundaries = store.list_boundaries(scope)?;
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
        Ok(Self {
            rules: rule_ids,
            resolutions,
            topics,
            questions,
            boundaries,
        })
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
