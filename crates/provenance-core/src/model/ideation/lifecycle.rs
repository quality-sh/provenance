use super::{DispositionDecision, PromotionState};
use crate::model::{
    validate_optional_confidence_score, Contribution, DispositionRatification, DispositionRecord,
    ProposalCard, SchemaVersion, ScopeId, StableId, SynthesisPacket,
};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

mod aggregate_validation;
mod assertion_validation;
mod legacy_validation;
mod lineage_validation;

pub use aggregate_validation::{
    ensure_supported_schema_version, validate_disposition_admissible, SUPPORTED_SCHEMA_VERSION,
};
pub use assertion_validation::packet_qualifies_proposal;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssertionId(StableId);

impl AssertionId {
    pub fn new(value: impl Into<String>) -> anyhow::Result<Self> {
        StableId::new(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub const fn as_stable_id(&self) -> &StableId {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionRecord {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: AssertionId,
    pub proposal_id: StableId,
    pub synthesis_packet_id: StableId,
    pub supporting_claim_ids: Vec<StableId>,
}

pub fn validate_assertion_intrinsic(assertion: &AssertionRecord) -> anyhow::Result<()> {
    anyhow::ensure!(
        !assertion.supporting_claim_ids.is_empty(),
        "assertion requires positive evidence"
    );
    let mut claims = BTreeSet::new();
    for claim_id in &assertion.supporting_claim_ids {
        anyhow::ensure!(
            claims.insert(claim_id.as_str()),
            "assertion supporting claim {} is repeated",
            claim_id.as_str()
        );
    }
    Ok(())
}

pub type Assertion = AssertionRecord;

#[derive(Clone, Copy)]
pub struct IdeationAggregate<'a> {
    pub legacy_policy: LegacyProposalPolicy,
    /// The resolved authority for recording dispositions: the legacy manifest
    /// allowlist inside the one-window compatibility period, or the rbac
    /// assignments whose `identity_type` must ratify each decision.
    pub ratification: DispositionRatification<'a>,
    pub contributions: &'a [Contribution],
    pub synthesis_packets: &'a [SynthesisPacket],
    pub proposals: &'a [ProposalCard],
    pub assertions: &'a [AssertionRecord],
    pub dispositions: &'a [DispositionRecord],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyProposalPolicy {
    ModernOnly,
    ShippedV1,
}

/// What an author may claim on a proposal row.
///
/// A modern proposal is written as `proposed` and nothing else. Every later
/// state is a verdict some other record reaches: a recorded assertion makes a
/// proposal asserted, a human disposition accepts, rejects or defers it. The
/// same holds for `duplicate_of` and `superseded_by`. Both name an outcome of
/// review, so an author may not write either onto the row being authored; only
/// a disposition record carries that authority.
///
/// The remaining checks are housekeeping on the row itself: `builds_on` may not
/// name the same assertion twice, and any confidence score must be in range.
#[rule("rule_proposal_authored_as_proposed")]
pub fn validate_proposal_intrinsic(proposal: &ProposalCard) -> anyhow::Result<()> {
    anyhow::ensure!(
        proposal.promotion_state == PromotionState::Proposed,
        "modern proposals must begin proposed; assertion and disposition derive later state"
    );
    anyhow::ensure!(
        proposal.duplicate_of.is_none() && proposal.superseded_by.is_none(),
        "proposal disposition links require an authoritative disposition record"
    );
    let mut lineage = BTreeSet::new();
    for assertion_id in &proposal.builds_on {
        anyhow::ensure!(
            lineage.insert(assertion_id.as_str()),
            "builds_on assertion {} is repeated",
            assertion_id.as_str()
        );
    }
    validate_optional_confidence_score(proposal.confidence)?;
    Ok(())
}

/// A proposal's real promotion state.
///
/// The `promotion_state` column on a proposal row is a claim, not a verdict.
/// A human's disposition decides the state; failing that, a recorded assertion
/// makes the proposal asserted. Every proposal read runs this, so no caller
/// sees a row's own claim once lifecycle records exist.
///
/// This half finds the records that belong to the proposal; the precedence it
/// applies to them is [`classify_proposal_state`].
#[rule("rule_proposal_effective_state")]
pub fn effective_proposal_state(
    proposal: &ProposalCard,
    assertions: &[AssertionRecord],
    dispositions: &[DispositionRecord],
) -> PromotionState {
    classify_proposal_state(
        proposal.promotion_state,
        dispositions
            .iter()
            .find(|record| record.proposal_id == proposal.id)
            .map(|record| record.decision),
        assertions
            .iter()
            .any(|record| record.proposal_id == proposal.id),
    )
}

/// The precedence half of [`effective_proposal_state`], over the finite axis
/// the decision ranges on: the state the row claims, the disposition outcome
/// if a human recorded one, and whether evidence was recorded.
fn classify_proposal_state(
    claimed: PromotionState,
    disposition: Option<DispositionDecision>,
    has_assertion: bool,
) -> PromotionState {
    if claimed != PromotionState::Proposed {
        return claimed;
    }
    match disposition {
        Some(DispositionDecision::Accepted) => PromotionState::Accepted,
        Some(DispositionDecision::Rejected) => PromotionState::Rejected,
        Some(DispositionDecision::Deferred) => PromotionState::Deferred,
        None if has_assertion => PromotionState::Asserted,
        None => PromotionState::Proposed,
    }
}

pub fn validate_ideation_aggregate(aggregate: IdeationAggregate<'_>) -> anyhow::Result<()> {
    aggregate_validation::validate(aggregate)
}

#[cfg(test)]
mod tests {
    use provenance_macros::verifies;
    use serde_json::json;

    use super::*;

    // The variant lists are derived from exhaustive matches so that adding a
    // PromotionState or DispositionDecision variant fails compilation until the
    // new variant joins the chain, keeping the exhaustion proof below complete.
    fn all_promotion_states() -> Vec<PromotionState> {
        let mut all = vec![PromotionState::Proposed];
        while let Some(next) = match all.last().unwrap() {
            PromotionState::Proposed => Some(PromotionState::Asserted),
            PromotionState::Asserted => Some(PromotionState::Accepted),
            PromotionState::Accepted => Some(PromotionState::Rejected),
            PromotionState::Rejected => Some(PromotionState::Deferred),
            PromotionState::Deferred => Some(PromotionState::Duplicate),
            PromotionState::Duplicate => Some(PromotionState::Superseded),
            PromotionState::Superseded => None,
        } {
            all.push(next);
        }
        all
    }

    fn all_disposition_outcomes() -> Vec<Option<DispositionDecision>> {
        let mut all = vec![None];
        while let Some(next) = match all.last().unwrap() {
            None => Some(Some(DispositionDecision::Accepted)),
            Some(DispositionDecision::Accepted) => Some(Some(DispositionDecision::Rejected)),
            Some(DispositionDecision::Rejected) => Some(Some(DispositionDecision::Deferred)),
            Some(DispositionDecision::Deferred) => None,
        } {
            all.push(next);
        }
        all
    }

    // Independent restatement of the precedence the rule fixes, written as a
    // priority list rather than as a copy of the function's branches:
    //   1. a row that has already left `proposed` is frozen at what it claims;
    //   2. otherwise the human's disposition decides;
    //   3. otherwise recorded evidence makes the proposal asserted;
    //   4. otherwise the proposal is still only proposed.
    // Must not be implemented by calling the classification.
    fn state_by_precedence(
        claimed: PromotionState,
        disposition: Option<DispositionDecision>,
        has_assertion: bool,
    ) -> PromotionState {
        let frozen_claim = (claimed != PromotionState::Proposed).then_some(claimed);
        let human_verdict = disposition.map(|decision| match decision {
            DispositionDecision::Accepted => PromotionState::Accepted,
            DispositionDecision::Rejected => PromotionState::Rejected,
            DispositionDecision::Deferred => PromotionState::Deferred,
        });
        let evidence_verdict = has_assertion.then_some(PromotionState::Asserted);
        frozen_claim
            .or(human_verdict)
            .or(evidence_verdict)
            .unwrap_or(PromotionState::Proposed)
    }

    #[test]
    #[verifies("rule_proposal_effective_state", exhaustion)]
    fn classification_matches_stated_precedence_on_every_case() {
        let mut cases = 0_usize;
        for claimed in all_promotion_states() {
            for disposition in all_disposition_outcomes() {
                for has_assertion in [false, true] {
                    assert_eq!(
                        classify_proposal_state(claimed, disposition, has_assertion),
                        state_by_precedence(claimed, disposition, has_assertion),
                        "row claiming {claimed:?} with disposition {disposition:?} and \
                         assertion present {has_assertion} disagrees with the stated precedence"
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(
            cases,
            all_promotion_states().len() * all_disposition_outcomes().len() * 2,
            "the classification axis is no longer states x outcomes x evidence"
        );
    }

    #[test]
    #[verifies("rule_proposal_effective_state", exhaustion)]
    fn a_recorded_disposition_makes_evidence_irrelevant() {
        for claimed in all_promotion_states() {
            for decision in all_disposition_outcomes().into_iter().flatten() {
                assert_eq!(
                    classify_proposal_state(claimed, Some(decision), false),
                    classify_proposal_state(claimed, Some(decision), true),
                    "assertion evidence changed the state of a row claiming {claimed:?} \
                     that a human already disposed as {decision:?}"
                );
            }
        }
    }

    fn proposal(id: &str) -> ProposalCard {
        serde_json::from_value(json!({
            "schema_version": 1, "scope_id": "default", "id": id, "proposal_key": id,
            "proposal_type": "question", "title": id, "summary": id,
            "traceability": {
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
            },
            "promotion_state": "proposed"
        }))
        .unwrap()
    }

    fn assertion_for(proposal_id: &str) -> AssertionRecord {
        serde_json::from_value(json!({
            "schema_version": 1, "scope_id": "default",
            "id": format!("assertion_{proposal_id}"), "proposal_id": proposal_id,
            "synthesis_packet_id": "synthesis_a", "supporting_claim_ids": ["claim_a"]
        }))
        .unwrap()
    }

    fn disposition_for(proposal_id: &str, decision: &str) -> DispositionRecord {
        serde_json::from_value(json!({
            "schema_version": 1, "scope_id": "default",
            "id": format!("disposition_{proposal_id}"), "proposal_id": proposal_id,
            "decision": decision, "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": "reviewer"}
        }))
        .unwrap()
    }

    #[test]
    #[verifies("rule_proposal_effective_state", examples)]
    fn another_proposals_disposition_does_not_decide_this_one() {
        assert_eq!(
            effective_proposal_state(
                &proposal("proposal_a"),
                &[],
                &[disposition_for("proposal_b", "accepted")],
            ),
            PromotionState::Proposed
        );
    }

    #[test]
    #[verifies("rule_proposal_effective_state", examples)]
    fn another_proposals_assertion_is_not_evidence_for_this_one() {
        assert_eq!(
            effective_proposal_state(&proposal("proposal_a"), &[assertion_for("proposal_b")], &[]),
            PromotionState::Proposed
        );
    }

    #[test]
    #[verifies("rule_proposal_effective_state", examples)]
    fn matching_rows_are_found_among_unrelated_rows() {
        assert_eq!(
            effective_proposal_state(
                &proposal("proposal_a"),
                &[assertion_for("proposal_b"), assertion_for("proposal_a")],
                &[
                    disposition_for("proposal_b", "rejected"),
                    disposition_for("proposal_a", "accepted"),
                ],
            ),
            PromotionState::Accepted
        );
    }

    // Independent restatement of what an author may claim, written from the
    // decision rather than from the validator's branches: the only authorable
    // row is a plain `proposed` one, carrying neither outcome link. Must not be
    // implemented by calling the validator.
    fn is_authorable(
        state: PromotionState,
        duplicate_of: Option<&str>,
        superseded_by: Option<&str>,
    ) -> bool {
        state == PromotionState::Proposed && duplicate_of.is_none() && superseded_by.is_none()
    }

    #[test]
    #[verifies("rule_proposal_authored_as_proposed", exhaustion)]
    fn only_a_plain_proposed_row_is_authorable() {
        let mut cases = 0_usize;
        for state in all_promotion_states() {
            for duplicate_of in [None, Some("proposal_original")] {
                for superseded_by in [None, Some("proposal_replacement")] {
                    let mut card = proposal("proposal_a");
                    card.promotion_state = state;
                    card.duplicate_of = duplicate_of.map(|id| StableId::new(id).unwrap());
                    card.superseded_by = superseded_by.map(|id| StableId::new(id).unwrap());
                    assert_eq!(
                        validate_proposal_intrinsic(&card).is_ok(),
                        is_authorable(state, duplicate_of, superseded_by),
                        "a row claiming {state:?} with duplicate_of {duplicate_of:?} and \
                         superseded_by {superseded_by:?} disagrees with what an author may claim"
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(
            cases,
            all_promotion_states().len() * 4,
            "the authorable axis is no longer states x duplicate_of x superseded_by"
        );
    }
}
