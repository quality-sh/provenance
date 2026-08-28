//! Topic and question workflow transitions, split out of
//! `shaping_writers`: claiming, releasing, closing, answering, and
//! updating existing records, plus the claim-eligibility law those
//! transitions enforce.

use super::shaping_writers::sort_artifact_links;
use super::{MutationAuth, ProposalDemand, StateStore, TopicClaim, UpdateQuestionInput};
use crate::shards;
use provenance_core::{
    Capability, Question, QuestionStatus, RbacClaim, ScopeId, StableId, Topic, TopicStatus,
};
use provenance_macros::rule;

#[cfg(test)]
mod claim_eligibility_tests;
impl StateStore {
    pub fn claim_topic(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        id: &StableId,
        actor: &str,
    ) -> anyhow::Result<TopicClaim> {
        let auth = MutationAuth::new(claim, Capability::Execute, scope_id);
        let actor = validated_actor(actor)?;
        let claimed_at = now_ms()?;
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
            let surfaced_proposals =
                self.surface_proposals(scope_id, &ProposalDemand::for_topic(&current_topic))?;
            let topic = self.update_topic(auth, scope_id, id, |topic| {
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

    pub fn release_topic(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Topic> {
        let auth = MutationAuth::new(claim, Capability::Execute, scope_id);
        self.update_topic(auth, scope_id, id, |topic| {
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

    pub fn close_topic(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Topic> {
        let auth = MutationAuth::new(claim, Capability::Execute, scope_id);
        self.update_topic(auth, scope_id, id, |topic| {
            topic.status = TopicStatus::Closed;
            clear_topic_claim_on_exit(topic);
            Ok(())
        })
    }

    pub fn claim_question(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        id: &StableId,
        actor: &str,
    ) -> anyhow::Result<Question> {
        let actor = validated_actor(actor)?;
        let claimed_at = now_ms()?;
        let auth = MutationAuth::new(claim, Capability::Execute, scope_id);
        self.mutate_question(auth, scope_id, id, |question| {
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

    pub fn release_question(
        &self,
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Question> {
        let auth = MutationAuth::new(claim, Capability::Execute, scope_id);
        self.mutate_question(auth, scope_id, id, |question| {
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
        claim: Option<&RbacClaim>,
        scope_id: &ScopeId,
        id: &StableId,
        answer: String,
        resolution_id: Option<StableId>,
    ) -> anyhow::Result<Question> {
        self.with_repository_publication(|| {
            self.write_question_answer(claim, scope_id, id, answer, resolution_id)
        })
    }

    fn write_question_answer(
        &self,
        claim: Option<&RbacClaim>,
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
        let auth = MutationAuth::new(claim, Capability::Execute, scope_id);
        self.mutate_question(auth, scope_id, id, |question| {
            question.status = QuestionStatus::Answered;
            question.answer = Some(answer);
            if resolution_id.is_some() {
                question.resolution_id = resolution_id;
            }
            clear_question_claim_on_exit(question);
            Ok(())
        })
    }

    pub fn update_question(
        &self,
        claim: Option<&RbacClaim>,
        input: UpdateQuestionInput,
    ) -> anyhow::Result<Question> {
        self.with_repository_publication(|| self.write_question_update(claim, input))
    }

    fn write_question_update(
        &self,
        claim: Option<&RbacClaim>,
        input: UpdateQuestionInput,
    ) -> anyhow::Result<Question> {
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
        let auth = MutationAuth::new(claim, Capability::Execute, &scope_id);
        self.mutate_question(auth, &scope_id, &id, |question| {
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
        auth: MutationAuth<'_>,
        scope_id: &ScopeId,
        id: &StableId,
        mutate: impl FnOnce(&mut Topic) -> anyhow::Result<()>,
    ) -> anyhow::Result<Topic> {
        let path = shards::topics_path(&self.layout, scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<Topic>| {
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
        auth: MutationAuth<'_>,
        scope_id: &ScopeId,
        id: &StableId,
        mutate: impl FnOnce(&mut Question) -> anyhow::Result<()>,
    ) -> anyhow::Result<Question> {
        let path = shards::questions_path(&self.layout, scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<Question>| {
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
pub(super) enum ShapingStatus {
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
pub(super) const fn claim_blocking_status(status: ShapingStatus) -> Option<&'static str> {
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
