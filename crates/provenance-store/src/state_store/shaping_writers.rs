use super::{
    CreateBoundaryInput, CreateQuestionInput, CreateTopicInput, ProposalDemand, StateStore,
    TopicClaim, UpdateQuestionInput,
};
use crate::{
    shards,
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::{
    Boundary, NodeType, Question, QuestionStatus, ScopeId, StableId, Topic, TopicStatus,
    SUPPORTED_SCHEMA_VERSION,
};

pub(super) mod artifact_links;
#[cfg(test)]
mod claim_eligibility_tests;
mod claims;
use claims::{claim_blocking_status, now_ms, validated_actor, ShapingStatus};
pub(super) use claims::{clear_question_claim_on_exit, clear_topic_claim_on_exit};

use artifact_links::sort_artifact_links;

impl StateStore {
    pub fn create_boundary(&self, input: CreateBoundaryInput) -> anyhow::Result<Boundary> {
        self.with_repository_publication(|| self.write_boundary(input))
    }

    fn write_boundary(&self, input: CreateBoundaryInput) -> anyhow::Result<Boundary> {
        let CreateBoundaryInput {
            scope_id,
            id,
            requirement_id,
            statement,
            source_ref,
        } = input;
        crate::write_error::ensure!(
            InvalidUpdate,
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        if let Some(source_ref) = &source_ref {
            crate::write_error::ensure!(
                InvalidUpdate,
                self.list_sources(&scope_id)?
                    .iter()
                    .any(|source| source.id == source_ref.source_id),
                "source does not exist"
            );
        }
        let path = shards::boundaries_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Boundary>| {
            let boundary = Boundary {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                requirement_id,
                statement,
                source_ref,
            };
            crate::write_error::ensure!(
                InvalidUpdate,
                !records.iter().any(|record| record.id == boundary.id),
                "boundary already exists"
            );
            records.push(boundary.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(boundary)
        })
    }

    pub fn create_topic(&self, input: CreateTopicInput) -> anyhow::Result<Topic> {
        self.with_repository_publication(|| self.write_topic(input))
    }

    fn write_topic(&self, input: CreateTopicInput) -> anyhow::Result<Topic> {
        let CreateTopicInput {
            scope_id,
            id,
            requirement_id,
            title,
            status,
            mut links,
        } = input;
        crate::write_error::ensure!(
            InvalidUpdate,
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        self.validate_artifact_links(&scope_id, &links)?;
        sort_artifact_links(&mut links);
        let path = shards::topics_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Topic>| {
            let topic = Topic {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                requirement_id,
                title,
                status,
                claimed_by: None,
                claimed_at: None,
                links,
            };
            crate::write_error::ensure!(
                InvalidUpdate,
                !records.iter().any(|record| record.id == topic.id),
                "topic already exists"
            );
            records.push(topic.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(topic)
        })
    }

    pub fn create_question(&self, input: CreateQuestionInput) -> anyhow::Result<Question> {
        self.with_repository_publication(|| self.write_question(input))
    }

    fn write_question(&self, input: CreateQuestionInput) -> anyhow::Result<Question> {
        let CreateQuestionInput {
            scope_id,
            id,
            topic_id,
            question,
            resolution_method,
            status,
            answer,
            mut links,
            resolution_id,
            contradicts,
        } = input;
        let topic = self
            .list_topics(&scope_id)?
            .into_iter()
            .find(|topic| topic.id == topic_id)
            .ok_or_else(|| {
                crate::write_error::SourceFailure::wrap(
                    crate::write_error::WriteFailure::MissingReference,
                    anyhow::anyhow!("topic does not exist"),
                )
            })?;
        if let Some(resolution_id) = &resolution_id {
            self.ensure_node_exists(
                &scope_id,
                NodeType::Resolution,
                resolution_id,
                "--resolution-id",
            )?;
        }
        if let Some(contradicted) = &contradicts {
            self.ensure_node_exists(
                &scope_id,
                NodeType::Requirement,
                contradicted,
                "--contradicts",
            )?;
        }
        self.validate_artifact_links(&scope_id, &links)?;
        sort_artifact_links(&mut links);
        let path = shards::questions_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Question>| {
            let question = Question {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                topic_id,
                requirement_id: topic.requirement_id,
                question,
                resolution_method,
                status,
                claimed_by: None,
                claimed_at: None,
                answer,
                links,
                resolution_id,
                contradicts,
            };
            crate::write_error::ensure!(
                InvalidUpdate,
                !records.iter().any(|record| record.id == question.id),
                "question already exists"
            );
            records.push(question.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(question)
        })
    }

    pub fn claim_topic<I, S>(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        actor: &str,
        changed_paths: I,
    ) -> anyhow::Result<TopicClaim>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let actor = validated_actor(actor)?;
        let claimed_at = now_ms()?;
        let changed_paths = changed_paths
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        self.with_repository_publication(|| {
            let current_topic = self
                .list_topics(scope_id)?
                .into_iter()
                .find(|topic| &topic.id == id)
                .ok_or_else(|| {
                    crate::write_error::SourceFailure::wrap(
                        crate::write_error::WriteFailure::MissingReference,
                        anyhow::anyhow!("topic does not exist"),
                    )
                })?;
            if let Some(blocking) =
                claim_blocking_status(ShapingStatus::Topic(current_topic.status))
            {
                return Err(SourceFailure::wrap(
                    WriteFailure::InvalidUpdate,
                    anyhow::anyhow!(
                        "topic {} is {blocking} and cannot be claimed",
                        current_topic.id.as_str()
                    ),
                ));
            }
            let mut demand = ProposalDemand::for_topic(&current_topic, changed_paths.clone());
            demand.extend_targets(self.topic_structural_territory(scope_id, &current_topic)?);
            let surfaced_proposals = self.surface_proposals(scope_id, &demand)?;
            let topic = self.update_topic(scope_id, id, |topic| {
                if let Some(holder) = &topic.claimed_by {
                    return Err(SourceFailure::wrap(
                        WriteFailure::InvalidUpdate,
                        anyhow::anyhow!(
                            "topic {} is already claimed by {holder}",
                            topic.id.as_str()
                        ),
                    ));
                }
                topic.claimed_by = Some(actor);
                topic.claimed_at = Some(claimed_at);
                Ok(())
            })?;
            Ok(TopicClaim {
                topic,
                surfaced_proposals,
            })
        })
    }

    pub fn release_topic(&self, scope_id: &ScopeId, id: &StableId) -> anyhow::Result<Topic> {
        self.update_topic(scope_id, id, |topic| {
            crate::write_error::ensure!(
                InvalidUpdate,
                topic.claimed_by.is_some(),
                "topic {} is not claimed",
                topic.id.as_str()
            );
            topic.claimed_by = None;
            topic.claimed_at = None;
            Ok(())
        })
    }

    pub fn close_topic(&self, scope_id: &ScopeId, id: &StableId) -> anyhow::Result<Topic> {
        self.update_topic(scope_id, id, |topic| {
            topic.status = TopicStatus::Closed;
            clear_topic_claim_on_exit(topic);
            Ok(())
        })
    }

    pub fn claim_question(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        actor: &str,
    ) -> anyhow::Result<Question> {
        let actor = validated_actor(actor)?;
        let claimed_at = now_ms()?;
        self.mutate_question(scope_id, id, |question| {
            let status = ShapingStatus::Question(question.status);
            if let Some(blocking) = claim_blocking_status(status) {
                return Err(SourceFailure::wrap(
                    WriteFailure::InvalidUpdate,
                    anyhow::anyhow!(
                        "question {} is {blocking} and cannot be claimed",
                        question.id.as_str()
                    ),
                ));
            }
            if let Some(holder) = &question.claimed_by {
                return Err(SourceFailure::wrap(
                    WriteFailure::InvalidUpdate,
                    anyhow::anyhow!(
                        "question {} is already claimed by {holder}",
                        question.id.as_str()
                    ),
                ));
            }
            question.claimed_by = Some(actor);
            question.claimed_at = Some(claimed_at);
            Ok(())
        })
    }

    pub fn release_question(&self, scope_id: &ScopeId, id: &StableId) -> anyhow::Result<Question> {
        self.mutate_question(scope_id, id, |question| {
            crate::write_error::ensure!(
                InvalidUpdate,
                question.claimed_by.is_some(),
                "question {} is not claimed",
                question.id.as_str()
            );
            question.claimed_by = None;
            question.claimed_at = None;
            Ok(())
        })
    }

    pub fn answer_question(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        answer: String,
        resolution_id: Option<StableId>,
    ) -> anyhow::Result<Question> {
        self.with_repository_publication(|| {
            self.write_question_answer(scope_id, id, answer, resolution_id)
        })
    }

    fn write_question_answer(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        answer: String,
        resolution_id: Option<StableId>,
    ) -> anyhow::Result<Question> {
        crate::write_error::ensure!(
            InvalidUpdate,
            !answer.trim().is_empty(),
            "answer must not be empty"
        );
        if let Some(resolution_id) = &resolution_id {
            crate::write_error::ensure!(
                InvalidUpdate,
                self.list_resolutions(scope_id)?
                    .iter()
                    .any(|resolution| &resolution.id == resolution_id),
                "resolution does not exist"
            );
        }
        self.mutate_question(scope_id, id, |question| {
            question.status = QuestionStatus::Answered;
            question.answer = Some(answer);
            if resolution_id.is_some() {
                question.resolution_id = resolution_id;
            }
            clear_question_claim_on_exit(question);
            Ok(())
        })
    }

    pub fn update_question(&self, input: UpdateQuestionInput) -> anyhow::Result<Question> {
        self.with_repository_publication(|| self.write_question_update(input))
    }

    fn write_question_update(&self, input: UpdateQuestionInput) -> anyhow::Result<Question> {
        let UpdateQuestionInput {
            scope_id,
            id,
            resolution_method,
            status,
            links,
            resolution_id,
        } = input;
        crate::write_error::ensure!(
            InvalidUpdate,
            resolution_method.is_some()
                || status.is_some()
                || links.is_some()
                || resolution_id.is_some(),
            "at least one question field must be updated"
        );
        self.edit_question(super::EditQuestionInput {
            scope_id,
            id,
            question: None,
            resolution_method,
            status,
            links,
            resolution_id,
            contradicts: None,
            clear_fields: Vec::new(),
        })
    }

    pub(super) fn update_topic(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        mutate: impl FnOnce(&mut Topic) -> anyhow::Result<()>,
    ) -> anyhow::Result<Topic> {
        let path = shards::topics_path(&self.layout, scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Topic>| {
            let topic = records
                .iter_mut()
                .find(|topic| &topic.id == id)
                .ok_or_else(|| {
                    crate::write_error::SourceFailure::wrap(
                        crate::write_error::WriteFailure::MissingReference,
                        anyhow::anyhow!("topic does not exist"),
                    )
                })?;
            mutate(topic)?;
            Ok(topic.clone())
        })
    }

    pub(super) fn mutate_question(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        mutate: impl FnOnce(&mut Question) -> anyhow::Result<()>,
    ) -> anyhow::Result<Question> {
        let path = shards::questions_path(&self.layout, scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Question>| {
            let question = records
                .iter_mut()
                .find(|question| &question.id == id)
                .ok_or_else(|| {
                    crate::write_error::SourceFailure::wrap(
                        crate::write_error::WriteFailure::MissingReference,
                        anyhow::anyhow!("question does not exist"),
                    )
                })?;
            mutate(question)?;
            Ok(question.clone())
        })
    }
}
