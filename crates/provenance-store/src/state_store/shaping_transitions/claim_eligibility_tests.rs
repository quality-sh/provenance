//! Exhaustion proofs for `rule_claim_eligibility` and its dual
//! `rule_claim_cleared_on_exit`.

use provenance_core::{QuestionStatus, TopicStatus};
use provenance_macros::verifies;

use super::{claim_blocking_status, claim_survives, ShapingStatus};

// The variant lists are derived from exhaustive matches so that adding a
// TopicStatus or QuestionStatus variant fails compilation until the new
// variant joins the chain, keeping the exhaustion proof below complete.
fn all_topic_statuses() -> Vec<TopicStatus> {
    let mut all = vec![TopicStatus::Open];
    while let Some(next) = match all.last().unwrap() {
        TopicStatus::Open => Some(TopicStatus::Explored),
        TopicStatus::Explored => Some(TopicStatus::Closed),
        TopicStatus::Closed => None,
    } {
        all.push(next);
    }
    all
}

fn all_question_statuses() -> Vec<QuestionStatus> {
    let mut all = vec![QuestionStatus::Open];
    while let Some(next) = match all.last().unwrap() {
        QuestionStatus::Open => Some(QuestionStatus::BlockedOnHuman),
        QuestionStatus::BlockedOnHuman => Some(QuestionStatus::Answered),
        QuestionStatus::Answered => None,
    } {
        all.push(next);
    }
    all
}

fn all_shaping_statuses() -> Vec<ShapingStatus> {
    all_topic_statuses()
        .into_iter()
        .map(ShapingStatus::Topic)
        .chain(
            all_question_statuses()
                .into_iter()
                .map(ShapingStatus::Question),
        )
        .collect()
}

/// The status word as the record itself writes it to disk.
fn stored_status_word(status: ShapingStatus) -> String {
    let value = match status {
        ShapingStatus::Topic(status) => serde_json::to_value(status),
        ShapingStatus::Question(status) => serde_json::to_value(status),
    }
    .unwrap();
    value.as_str().unwrap().to_string()
}

// Independent restatement of the decision: a shaping record stops wanting a
// worker once it reaches a state that expects none - it is finished (closed,
// answered) or it is waiting on a human. The refusal names that state with the
// same word the record stores. Must not be implemented by calling
// claim_blocking_status.
const STATES_THAT_WANT_NO_WORKER: [&str; 3] = ["closed", "answered", "blocked_on_human"];

fn claim_refusal_oracle(status: ShapingStatus) -> Option<String> {
    let word = stored_status_word(status);
    STATES_THAT_WANT_NO_WORKER
        .contains(&word.as_str())
        .then_some(word)
}

#[test]
#[verifies("rule_claim_eligibility", exhaustion)]
fn every_shaping_status_matches_the_claim_decision() {
    for status in all_shaping_statuses() {
        assert_eq!(
            claim_blocking_status(status).map(ToString::to_string),
            claim_refusal_oracle(status),
            "claim decision disagrees with the oracle for {status:?}"
        );
    }
}

// The entry rule and its dual are written independently, so nothing but this
// test keeps them in step: a status that refuses a new claim must also refuse
// to carry the claim it already has, and a status that grants one must keep
// it. Any disagreement would leave a worker holding a claim the store would
// not have granted, or drop a claim on live work.
#[test]
#[verifies("rule_claim_cleared_on_exit", exhaustion)]
fn a_claim_survives_exactly_the_statuses_that_could_grant_it() {
    for status in all_shaping_statuses() {
        assert_eq!(
            claim_survives(status),
            claim_blocking_status(status).is_none(),
            "entry and exit disagree for {status:?}"
        );
    }
}

#[test]
#[verifies("rule_claim_eligibility", exhaustion)]
fn every_status_enum_keeps_a_claimable_state() {
    assert!(all_topic_statuses()
        .into_iter()
        .any(|status| claim_blocking_status(ShapingStatus::Topic(status)).is_none()));
    assert!(all_question_statuses()
        .into_iter()
        .any(|status| claim_blocking_status(ShapingStatus::Question(status)).is_none()));
}
