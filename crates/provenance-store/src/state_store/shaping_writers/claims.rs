use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{Question, QuestionStatus, StableId, Topic, TopicStatus};
use provenance_macros::rule;

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
        ShapingStatus::Topic(TopicStatus::Archived)
        | ShapingStatus::Question(QuestionStatus::Archived) => Some("archived"),
        ShapingStatus::Question(QuestionStatus::BlockedOnHuman) => Some("blocked_on_human"),
        ShapingStatus::Question(QuestionStatus::Answered) => Some("answered"),
    }
}

/// A claim does not survive the record leaving the claimable state.
///
/// The dual of [`claim_blocking_status`]: that one guards the entry, this one
/// guards the exit, so the write that closes a topic or answers a question
/// takes the claim with it and nobody is left holding work that is over. The
/// two read the same statuses and agree on every one of them, which is
/// what makes a held claim mean the same thing as a grantable one.
#[rule("rule_claim_cleared_on_exit")]
pub(super) const fn claim_survives(status: ShapingStatus) -> bool {
    matches!(
        status,
        ShapingStatus::Topic(TopicStatus::Open | TopicStatus::Explored)
            | ShapingStatus::Question(QuestionStatus::Open)
    )
}

/// Drops a topic's claim when the status just written no longer holds one.
pub(in crate::state_store) fn clear_topic_claim_on_exit(topic: &mut Topic) {
    if !claim_survives(ShapingStatus::Topic(topic.status)) {
        topic.claimed_by = None;
        topic.claimed_at = None;
    }
}

/// Drops a question's claim when the status just written no longer holds one.
pub(in crate::state_store) fn clear_question_claim_on_exit(question: &mut Question) {
    if !claim_survives(ShapingStatus::Question(question.status)) {
        question.claimed_by = None;
        question.claimed_at = None;
    }
}

/// A held claim belongs to one actor. The holder's repeated claim and the
/// holder's release succeed; another actor's claim or release is refused with
/// a conflict that names the holder.
#[rule("rule_claim_holder_exclusive")]
pub(super) fn ensure_claim_holder(
    kind: &str,
    id: &StableId,
    holder: Option<&str>,
    actor: &str,
) -> anyhow::Result<()> {
    match holder {
        Some(holder) if holder != actor => Err(SourceFailure::wrap(
            WriteFailure::ClaimConflict {
                holder: holder.to_owned(),
            },
            anyhow::anyhow!("{kind} {} is already claimed by {holder}", id.as_str()),
        )),
        _ => Ok(()),
    }
}

pub(super) fn validated_actor(actor: &str) -> anyhow::Result<String> {
    crate::write_error::ensure_actor(actor)?;
    Ok(actor.trim().to_string())
}

pub(super) fn now_ms() -> anyhow::Result<i64> {
    Ok(i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?)
}
