use super::{
    CreateBoundaryInput, CreateQuestionInput, CreateTopicInput, ProposalDemand, StateStore,
    TopicClaim, UpdateQuestionInput,
};
use crate::shards;
use provenance_core::{
    Boundary, Question, QuestionStatus, ScopeId, StableId, Topic, TopicStatus,
    SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;

mod artifact_links;
#[cfg(test)]
mod claim_eligibility_tests;

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
        anyhow::ensure!(
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        if let Some(source_ref) = &source_ref {
            anyhow::ensure!(
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
            anyhow::ensure!(
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
        anyhow::ensure!(
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
            anyhow::ensure!(
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
        } = input;
        let topic = self
            .list_topics(&scope_id)?
            .into_iter()
            .find(|topic| topic.id == topic_id)
            .ok_or_else(|| anyhow::anyhow!("topic does not exist"))?;
        if let Some(resolution_id) = &resolution_id {
            anyhow::ensure!(
                self.list_resolutions(&scope_id)?
                    .iter()
                    .any(|resolution| &resolution.id == resolution_id),
                "resolution does not exist"
            );
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
            };
            anyhow::ensure!(
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
                .ok_or_else(|| anyhow::anyhow!("topic does not exist"))?;
            if let Some(blocking) =
                claim_blocking_status(ShapingStatus::Topic(current_topic.status))
            {
                anyhow::bail!(
                    "topic {} is {blocking} and cannot be claimed",
                    current_topic.id.as_str()
                );
            }
            let mut demand = ProposalDemand::for_topic(&current_topic, changed_paths.clone());
            demand.extend_targets(self.topic_structural_territory(scope_id, &current_topic)?);
            let surfaced_proposals = self.surface_proposals(scope_id, &demand)?;
            let topic = self.update_topic(scope_id, id, |topic| {
                if let Some(holder) = &topic.claimed_by {
                    anyhow::bail!("topic {} is already claimed by {holder}", topic.id.as_str());
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
            anyhow::ensure!(
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
                anyhow::bail!(
                    "question {} is {blocking} and cannot be claimed",
                    question.id.as_str()
                );
            }
            if let Some(holder) = &question.claimed_by {
                anyhow::bail!(
                    "question {} is already claimed by {holder}",
                    question.id.as_str()
                );
            }
            question.claimed_by = Some(actor);
            question.claimed_at = Some(claimed_at);
            Ok(())
        })
    }

    pub fn release_question(&self, scope_id: &ScopeId, id: &StableId) -> anyhow::Result<Question> {
        self.mutate_question(scope_id, id, |question| {
            anyhow::ensure!(
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
        anyhow::ensure!(!answer.trim().is_empty(), "answer must not be empty");
        if let Some(resolution_id) = &resolution_id {
            anyhow::ensure!(
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
            mut links,
            resolution_id,
        } = input;
        anyhow::ensure!(
            resolution_method.is_some()
                || status.is_some()
                || links.is_some()
                || resolution_id.is_some(),
            "at least one question field must be updated"
        );
        if let Some(resolution_id) = &resolution_id {
            anyhow::ensure!(
                self.list_resolutions(&scope_id)?
                    .iter()
                    .any(|resolution| &resolution.id == resolution_id),
                "resolution does not exist"
            );
        }
        if let Some(links) = &mut links {
            self.validate_artifact_links(&scope_id, links)?;
            sort_artifact_links(links);
        }
        self.mutate_question(&scope_id, &id, |question| {
            if let Some(resolution_method) = resolution_method {
                question.resolution_method = resolution_method;
            }
            if let Some(status) = status {
                anyhow::ensure!(
                    status != QuestionStatus::Answered || question.answer.is_some(),
                    "use questions answer --answer to answer a question"
                );
                question.status = status;
                clear_question_claim_on_exit(question);
            }
            if let Some(links) = links {
                question.links = links;
            }
            if let Some(resolution_id) = resolution_id {
                question.resolution_id = Some(resolution_id);
            }
            Ok(())
        })
    }

    fn update_topic(
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
                .ok_or_else(|| anyhow::anyhow!("topic does not exist"))?;
            mutate(topic)?;
            Ok(topic.clone())
        })
    }

    fn mutate_question(
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
                .ok_or_else(|| anyhow::anyhow!("question does not exist"))?;
            mutate(question)?;
            Ok(question.clone())
        })
    }
}

/// The status of a shaping record an actor is trying to claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapingStatus {
    Topic(TopicStatus),
    Question(QuestionStatus),
}

/// Shaping work may be claimed only while its record is still open for work.
///
/// A closed topic and an answered question are finished, and a question
/// blocked on a human is waiting on someone the claimant is not; none of the
/// three wants a worker. Every other status is live work, so an explored topic
/// stays claimable. The decision reads the status alone: who already holds the
/// claim is a separate question, answered after this one.
///
/// Returns the status word to name in the refusal, or `None` when the record
/// may be claimed.
#[rule("rule_claim_eligibility")]
const fn claim_blocking_status(status: ShapingStatus) -> Option<&'static str> {
    match status {
        ShapingStatus::Topic(TopicStatus::Open | TopicStatus::Explored)
        | ShapingStatus::Question(QuestionStatus::Open) => None,
        ShapingStatus::Topic(TopicStatus::Closed) => Some("closed"),
        ShapingStatus::Question(QuestionStatus::BlockedOnHuman) => Some("blocked_on_human"),
        ShapingStatus::Question(QuestionStatus::Answered) => Some("answered"),
    }
}

/// A claim does not survive the record leaving the claimable state.
///
/// The dual of [`claim_blocking_status`]: that one guards the entry, this one
/// guards the exit, so the write that closes a topic or answers a question
/// takes the claim with it and nobody is left holding work that is over. The
/// two read the same six statuses and agree on every one of them, which is
/// what makes a held claim mean the same thing as a grantable one.
#[rule("rule_claim_cleared_on_exit")]
const fn claim_survives(status: ShapingStatus) -> bool {
    matches!(
        status,
        ShapingStatus::Topic(TopicStatus::Open | TopicStatus::Explored)
            | ShapingStatus::Question(QuestionStatus::Open)
    )
}

/// Drops a topic's claim when the status just written no longer holds one.
fn clear_topic_claim_on_exit(topic: &mut Topic) {
    if !claim_survives(ShapingStatus::Topic(topic.status)) {
        topic.claimed_by = None;
        topic.claimed_at = None;
    }
}

/// Drops a question's claim when the status just written no longer holds one.
fn clear_question_claim_on_exit(question: &mut Question) {
    if !claim_survives(ShapingStatus::Question(question.status)) {
        question.claimed_by = None;
        question.claimed_at = None;
    }
}

fn validated_actor(actor: &str) -> anyhow::Result<String> {
    let actor = actor.trim();
    anyhow::ensure!(!actor.is_empty(), "actor must not be empty");
    Ok(actor.to_string())
}

fn now_ms() -> anyhow::Result<i64> {
    Ok(i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}
