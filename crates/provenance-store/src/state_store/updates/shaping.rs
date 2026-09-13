use super::{
    inputs::{EditQuestionInput, QuestionClearField, UpdateTopicInput},
    invalid, optional, required_text, set,
};
use crate::state_store::shaping_writers::{
    clear_question_claim_on_exit, clear_topic_claim_on_exit,
};
use crate::state_store::StateStore;
use provenance_core::{NodeType, Question, QuestionStatus, Topic};

impl StateStore {
    pub fn edit_topic(&self, input: UpdateTopicInput) -> anyhow::Result<Topic> {
        self.with_repository_publication(|| {
            if let Some(title) = &input.title {
                required_text(title)?;
            }
            let links = self.checked_update_links(&input.scope_id, input.links)?;
            self.update_topic(&input.scope_id, &input.id, |topic| {
                set(&mut topic.title, input.title);
                set(&mut topic.status, input.status);
                set(&mut topic.links, links);
                clear_topic_claim_on_exit(topic);
                Ok(())
            })
        })
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
            self.mutate_question(&input.scope_id, &input.id, |question| {
                if input.status == Some(QuestionStatus::Answered) && question.answer.is_none() {
                    return Err(invalid(
                        "use questions answer --answer to answer a question",
                    ));
                }
                set(&mut question.question, input.question);
                set(&mut question.resolution_method, input.resolution_method);
                set(&mut question.status, input.status);
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
            })
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
