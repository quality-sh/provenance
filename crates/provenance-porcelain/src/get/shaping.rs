//! A bounded read of the claims, methods, and prerequisites for one Topic.
use provenance_core::{Question, QuestionStatus, ResolutionMethod, StableId, Topic, TopicStatus};
use provenance_macros::rule;
use serde::Serialize;

const SUMMARY_BYTES: usize = 65_536;

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Claim {
    pub claimed_by: Option<String>,
    pub claimed_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct QuestionState {
    pub id: StableId,
    pub resolution_method: ResolutionMethod,
    pub status: QuestionStatus,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<i64>,
    pub depends_on: Vec<StableId>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct BlockedQuestion {
    pub id: StableId,
    pub reasons: Vec<BlockedReason>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum BlockedReason {
    TopicInactive,
    Prerequisite {
        id: StableId,
        status: Option<QuestionStatus>,
    },
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct SessionSummary {
    pub topic_claim: Option<Claim>,
    pub questions: Vec<QuestionState>,
    pub ready_ids: Vec<StableId>,
    pub blocked: Vec<BlockedQuestion>,
    pub complete: bool,
}

impl SessionSummary {
    /// Only answered prerequisites permit an open Question to be ready.
    #[rule("rule_shaping_question_ready")]
    pub fn from_questions(topic: &Topic, questions: &[Question], complete: bool) -> Self {
        let mut questions: Vec<_> = questions
            .iter()
            .filter(|question| question.topic_id == topic.id)
            .collect();
        questions.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        let mut summary = Self {
            topic_claim: Some(Claim {
                claimed_by: topic.claimed_by.clone(),
                claimed_at: topic.claimed_at,
            }),
            questions: Vec::new(),
            ready_ids: Vec::new(),
            blocked: Vec::new(),
            complete,
        };
        for question in &questions {
            summary.questions.push(QuestionState {
                id: question.id.clone(),
                resolution_method: question.resolution_method,
                status: question.status,
                claimed_by: question.claimed_by.clone(),
                claimed_at: question.claimed_at,
                depends_on: question.depends_on.clone(),
            });
            if question.status != QuestionStatus::Open {
                continue;
            }
            let mut reasons = Vec::new();
            if matches!(topic.status, TopicStatus::Closed | TopicStatus::Archived) {
                reasons.push(BlockedReason::TopicInactive);
            }
            for dependency in &question.depends_on {
                let status = questions
                    .iter()
                    .find(|candidate| candidate.id == *dependency)
                    .map(|candidate| candidate.status);
                if status != Some(QuestionStatus::Answered) {
                    reasons.push(BlockedReason::Prerequisite {
                        id: dependency.clone(),
                        status,
                    });
                }
            }
            if reasons.is_empty() {
                summary.ready_ids.push(question.id.clone());
            } else {
                summary.blocked.push(BlockedQuestion {
                    id: question.id.clone(),
                    reasons,
                });
            }
        }
        summary.bound();
        summary
    }

    fn bound(&mut self) {
        while serde_json::to_vec(self).expect("summary is JSON").len() > SUMMARY_BYTES {
            self.complete = false;
            self.ready_ids.clear();
            self.blocked.clear();
            if self.questions.pop().is_none() {
                self.topic_claim = None;
            }
        }
    }
}
