use super::{
    inputs::{EditQuestionInput, QuestionClearField, UpdateTopicInput},
    invalid, optional, required_text, set,
};
use crate::review::NativeRecordBatch;
use crate::state_store::shaping_writers::{
    clear_question_claim_on_exit, clear_topic_claim_on_exit,
};
use crate::state_store::StateStore;
use provenance_core::{NodeType, Question, QuestionStatus, Topic, TopicStatus};

impl StateStore {
    pub fn edit_topic(&self, input: UpdateTopicInput) -> anyhow::Result<Topic> {
        if input.status == Some(TopicStatus::Archived) {
            return self
                .with_native_record_batch(|batch| batch.store().write_topic_archive(batch, input));
        }
        self.with_repository_publication(|| self.write_topic_update(input))
    }

    fn write_topic_archive(
        &self,
        batch: &NativeRecordBatch<'_>,
        input: UpdateTopicInput,
    ) -> anyhow::Result<Topic> {
        if let Some(title) = &input.title {
            required_text(title)?;
        }
        let links = self.checked_update_links(&input.scope_id, input.links)?;
        let archived_in_commit = input.archived_in_commit.clone();
        let topic_path = crate::shards::topics_path(&self.layout, &input.scope_id);
        let topic = batch.mutate_one(
            &topic_path,
            input.expected_etag.as_deref(),
            |topics: &mut Vec<Topic>| {
                let topic = topics
                    .iter_mut()
                    .find(|topic| topic.id == input.id)
                    .ok_or_else(|| invalid("topic does not exist"))?;
                set(&mut topic.title, input.title);
                set(&mut topic.status, input.status);
                set(&mut topic.links, links);
                topic.archived_in_commit.clone_from(&archived_in_commit);
                clear_topic_claim_on_exit(topic);
                Ok(topic.clone())
            },
        )?;
        crate::test_probes::at("topic_archive_after_topic")?;
        let stamp = topic
            .archived_in_commit
            .as_ref()
            .ok_or_else(|| invalid("archived_in_commit is required for an archived Topic"))?;
        let questions_path = crate::shards::questions_path(&self.layout, &input.scope_id);
        batch.mutate_all(&questions_path, |questions: &mut Vec<Question>| {
            for question in questions.iter_mut().filter(|question| {
                question.topic_id == input.id && question.status != QuestionStatus::Archived
            }) {
                question.status = QuestionStatus::Archived;
                question.archived_in_commit = Some(stamp.clone());
                clear_question_claim_on_exit(question);
                crate::test_probes::at("topic_archive_question_changed")?;
            }
            Ok(())
        })?;
        Ok(topic)
    }

    fn write_topic_update(&self, input: UpdateTopicInput) -> anyhow::Result<Topic> {
        if let Some(title) = &input.title {
            required_text(title)?;
        }
        let links = self.checked_update_links(&input.scope_id, input.links)?;
        let archived_in_commit = input.archived_in_commit.clone();
        let topic = self.update_topic_with_etag(
            &input.scope_id,
            &input.id,
            input.expected_etag.as_deref(),
            |topic| {
                set(&mut topic.title, input.title);
                set(&mut topic.status, input.status);
                set(&mut topic.links, links);
                if archived_in_commit.is_some() {
                    topic.archived_in_commit.clone_from(&archived_in_commit);
                }
                clear_topic_claim_on_exit(topic);
                Ok(())
            },
        )?;
        Ok(topic)
    }

    pub fn edit_question(&self, input: EditQuestionInput) -> anyhow::Result<Question> {
        self.with_repository_publication(|| {
            if let Some(question) = &input.question {
                required_text(question)?;
            }
            if let Some(id) = &input.resolution_id {
                self.ensure_node_exists(
                    &input.scope_id,
                    NodeType::Resolution,
                    id,
                    "resolution_id",
                )?;
            }
            if let Some(id) = &input.contradicts {
                self.ensure_node_exists(&input.scope_id, NodeType::Requirement, id, "contradicts")?;
            }
            let links = self.checked_update_links(&input.scope_id, input.links)?;
            self.mutate_question_with_etag(
                &input.scope_id,
                &input.id,
                input.expected_etag.as_deref(),
                |question| {
                    if input.status == Some(QuestionStatus::Answered) && question.answer.is_none() {
                        return Err(invalid(
                            "use questions answer --answer to answer a question",
                        ));
                    }
                    set(&mut question.question, input.question);
                    set(&mut question.resolution_method, input.resolution_method);
                    set(&mut question.status, input.status);
                    if input.archived_in_commit.is_some() {
                        question.archived_in_commit = input.archived_in_commit;
                    }
                    set(&mut question.links, links);
                    optional(
                        &mut question.resolution_id,
                        input.resolution_id,
                        input
                            .clear_fields
                            .contains(&QuestionClearField::ResolutionId),
                    )?;
                    optional(
                        &mut question.contradicts,
                        input.contradicts,
                        input
                            .clear_fields
                            .contains(&QuestionClearField::Contradicts),
                    )?;
                    clear_question_claim_on_exit(question);
                    Ok(())
                },
            )
        })
    }

    fn checked_update_links(
        &self,
        scope: &provenance_core::ScopeId,
        mut links: Option<Vec<provenance_core::ArtifactLink>>,
    ) -> anyhow::Result<Option<Vec<provenance_core::ArtifactLink>>> {
        if let Some(links) = &mut links {
            self.validate_artifact_links(scope, links)?;
            crate::state_store::shaping_writers::artifact_links::sort_artifact_links(links);
        }
        Ok(links)
    }
}
