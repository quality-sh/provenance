//! Archive membership and parent cleanup use one complete scope snapshot.
use std::collections::BTreeSet;

use provenance_core::{
    ArchivedStamp, Question, QuestionStatus, Requirement, RequirementStatus, Resolution,
    ResolutionStatus, Rule, RuleStatus, ScopeId, StableId, Topic, TopicStatus,
};
use provenance_macros::rule;

use crate::{shards, state_store::StateStore};

impl StateStore {
    /// Archives affected descendants after all surviving parents are known.
    #[rule("rule_archive_records_each_commit")]
    #[rule("rule_archive_keeps_archived_parent_links")]
    #[rule("rule_archive_cascades_without_surviving_parents")]
    #[rule("rule_archive_completes_cascade_in_one_operation")]
    #[rule("rule_live_dependents_have_live_parents")]
    #[rule("rule_archive_does_not_create_roots")]
    pub(crate) fn archive_dependents(
        &self,
        scope: &ScopeId,
        selected: &StableId,
        stamp: &ArchivedStamp,
    ) -> anyhow::Result<()> {
        let requirements = self.list_requirements(scope)?;
        let mut cascade = BTreeSet::from([selected.as_str().to_owned()]);
        let archived = requirements
            .iter()
            .filter(|r| r.status == RequirementStatus::Archived)
            .map(|r| r.id.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        loop {
            let mut changed = false;
            for record in &requirements {
                if record.status == RequirementStatus::Archived
                    || cascade.contains(record.id.as_str())
                {
                    continue;
                }
                if record
                    .refines
                    .as_ref()
                    .is_some_and(|parent| cascade.contains(parent.as_str()))
                {
                    changed |= cascade.insert(record.id.as_str().to_owned());
                }
            }
            if !changed {
                break;
            }
        }
        let unavailable = archived.union(&cascade).cloned().collect::<BTreeSet<_>>();
        self.with_native_record_batch(|batch| {
            let store = batch.store();
            batch.mutate_all(
                &shards::requirements_path(&store.layout, scope),
                |records: &mut Vec<Requirement>| {
                    for record in records
                        .iter_mut()
                        .filter(|r| cascade.contains(r.id.as_str()))
                    {
                        if record.status != RequirementStatus::Archived {
                            record.status = RequirementStatus::Archived;
                            record.archived_in_commit = Some(stamp.clone());
                        }
                    }
                    Ok(())
                },
            )?;
            batch.mutate_all(
                &shards::resolutions_path(&store.layout, scope),
                |records: &mut Vec<Resolution>| {
                    for record in records
                        .iter_mut()
                        .filter(|r| r.status != ResolutionStatus::Archived)
                    {
                        if affected(&record.requirement_ids, &cascade) {
                            if no_survivors(&record.requirement_ids, &unavailable) {
                                record.status = ResolutionStatus::Archived;
                                record.archived_in_commit = Some(stamp.clone());
                            } else {
                                prune(&mut record.requirement_ids, &cascade);
                            }
                        }
                    }
                    Ok(())
                },
            )?;
            batch.mutate_all(
                &shards::rules_path(&store.layout, scope),
                |records: &mut Vec<Rule>| {
                    for record in records
                        .iter_mut()
                        .filter(|r| r.status != RuleStatus::Archived)
                    {
                        if affected(&record.requirement_ids, &cascade) {
                            if no_survivors(&record.requirement_ids, &unavailable) {
                                record.status = RuleStatus::Archived;
                                record.archived_in_commit = Some(stamp.clone());
                            } else {
                                prune(&mut record.requirement_ids, &cascade);
                            }
                        }
                    }
                    Ok(())
                },
            )?;
            archive_shaping(batch, scope, &cascade, stamp)
        })
    }
}

fn affected(parents: &[StableId], cascade: &BTreeSet<String>) -> bool {
    parents
        .iter()
        .any(|parent| cascade.contains(parent.as_str()))
}

/// An archived parent cannot preserve a live child.
fn no_survivors(parents: &[StableId], unavailable: &BTreeSet<String>) -> bool {
    parents
        .iter()
        .all(|parent| unavailable.contains(parent.as_str()))
}

/// Removes only the owning links to parents archived by this operation.
#[rule("rule_archive_preserves_surviving_children")]
#[rule("rule_archive_prunes_only_parent_edge")]
fn prune(parents: &mut Vec<StableId>, cascade: &BTreeSet<String>) {
    parents.retain(|parent| !cascade.contains(parent.as_str()));
}

fn archive_shaping(
    batch: &crate::review::NativeRecordBatch<'_>,
    scope: &ScopeId,
    cascade: &BTreeSet<String>,
    stamp: &ArchivedStamp,
) -> anyhow::Result<()> {
    let store = batch.store();
    let mut topics = BTreeSet::new();
    batch.mutate_all(
        &shards::topics_path(&store.layout, scope),
        |records: &mut Vec<Topic>| {
            for record in records
                .iter_mut()
                .filter(|r| cascade.contains(r.requirement_id.as_str()))
            {
                topics.insert(record.id.as_str().to_owned());
                if record.status != TopicStatus::Archived {
                    record.status = TopicStatus::Archived;
                    record.archived_in_commit = Some(stamp.clone());
                    super::super::shaping_writers::clear_topic_claim_on_exit(record);
                }
            }
            Ok(())
        },
    )?;
    batch.mutate_all(
        &shards::questions_path(&store.layout, scope),
        |records: &mut Vec<Question>| {
            for record in records.iter_mut().filter(|r| {
                topics.contains(r.topic_id.as_str()) && r.status != QuestionStatus::Archived
            }) {
                record.status = QuestionStatus::Archived;
                record.archived_in_commit = Some(stamp.clone());
                super::super::shaping_writers::clear_question_claim_on_exit(record);
            }
            Ok(())
        },
    )
}
