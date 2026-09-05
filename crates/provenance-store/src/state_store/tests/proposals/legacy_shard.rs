//! Verification for the retired `promotion_decisions.jsonl` admission gate.
//!
//! The gate decides on two things: whether the rows in front of it are the
//! shipped-v1 audit, which is a property of the whole set and is answered by
//! `provenance-core`'s frozen fingerprint, and whether each row's proposal has
//! already left `proposed`. The exhaustion below walks the shipped audit and
//! four ways of not being it against every promotion state. The examples pin
//! the case the gate exists for: a new decision appended beside genuine
//! history, which the membership test refuses even when its proposal has been
//! moved to a terminal state to make it look like history.

use crate::state_store::ideation_batches::validate_legacy_disposition_shard;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    DispositionActor, DispositionDecision, DispositionRecord, IdeationTarget, IdeationTargetType,
    IdentityType, PromotionState, ProposalCard, ProposalTraceability, ProposalType, ScopeId,
    StableId,
};
use provenance_macros::verifies;
use std::collections::BTreeSet;

// Derived from an exhaustive match so that adding a PromotionState variant
// fails compilation until the new variant joins the chain, keeping the
// exhaustion proof below complete.
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

// How a shard can stand in relation to the shipped audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShardCase {
    Empty,
    ShippedAudit,
    Appended,
    Omitted,
    Edited,
    Forged,
}

fn all_shard_cases() -> Vec<ShardCase> {
    let mut all = vec![ShardCase::Empty];
    while let Some(next) = match all.last().unwrap() {
        ShardCase::Empty => Some(ShardCase::ShippedAudit),
        ShardCase::ShippedAudit => Some(ShardCase::Appended),
        ShardCase::Appended => Some(ShardCase::Omitted),
        ShardCase::Omitted => Some(ShardCase::Edited),
        ShardCase::Edited => Some(ShardCase::Forged),
        ShardCase::Forged => None,
    } {
        all.push(next);
    }
    all
}

fn shard_rows(case: ShardCase) -> Vec<DispositionRecord> {
    match case {
        ShardCase::Empty => Vec::new(),
        ShardCase::ShippedAudit => shipped_audit(),
        ShardCase::Appended => {
            let mut rows = shipped_audit();
            rows.push(legacy_row("disposition_new", "proposal_open"));
            rows
        }
        ShardCase::Omitted => {
            let mut rows = shipped_audit();
            rows.remove(0);
            rows
        }
        ShardCase::Edited => {
            let mut rows = shipped_audit();
            rows[0].rationale = "Rewritten after the fact.".into();
            rows
        }
        ShardCase::Forged => vec![legacy_row("disposition_forged", "proposal_forged")],
    }
}

#[test]
#[verifies("rule_legacy_shard_frozen", exhaustion)]
fn admits_only_the_shipped_audit_over_proposals_that_left_proposed() {
    for case in all_shard_cases() {
        for state in all_promotion_states() {
            let rows = shard_rows(case);
            let proposals = proposals_for(&rows, state);
            let admitted = validate_legacy_disposition_shard(&rows, &proposals).is_ok();

            // Independent restatement of the decision: a shard claiming
            // nothing is admitted, and a shard claiming anything is admitted
            // only when what it claims is the shipped audit recording history
            // that is over.
            let allowed = case == ShardCase::Empty
                || (case == ShardCase::ShippedAudit && state != PromotionState::Proposed);
            assert_eq!(
                admitted, allowed,
                "legacy shard gate admitted={admitted} for {case:?} over proposals in {state:?}"
            );
        }
    }
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn an_empty_shard_is_admitted() {
    validate_legacy_disposition_shard(&[], &[]).unwrap();
    validate_legacy_disposition_shard(&[], &[proposal("proposal_open", PromotionState::Proposed)])
        .unwrap();
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn refuses_a_row_naming_no_proposal_at_all() {
    let rows = [legacy_row("disposition_orphan", "proposal_missing")];
    let error = validate_legacy_disposition_shard(&rows, &[])
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "deprecated promotion_decisions.jsonl accepts only the frozen shipped-v1 disposition audit"
    );
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn refuses_a_new_decision_appended_beside_genuine_history() {
    let history = shipped_audit();
    let proposals = proposals_for(&history, PromotionState::Accepted);
    validate_legacy_disposition_shard(&history, &proposals).unwrap();

    // The row is appended and its proposal is written terminal in the same
    // breath, which is what the promotion-state test alone used to admit.
    let mut appended = history;
    appended.push(legacy_row("disposition_new", "proposal_open"));
    let proposals = proposals_for(&appended, PromotionState::Accepted);
    let error = validate_legacy_disposition_shard(&appended, &proposals)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("frozen shipped-v1 disposition audit"),
        "{error}"
    );
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn refuses_an_edited_or_deleted_row() {
    // Membership is a property of the whole set, so the gate now answers for
    // every field of every row, not only for the proposal each row names.
    let history = shipped_audit();
    let proposals = proposals_for(&history, PromotionState::Accepted);

    let mut edited = history.clone();
    edited[0].rationale = "Rewritten after the fact.".into();
    edited[0].decision = DispositionDecision::Rejected;
    assert!(validate_legacy_disposition_shard(&edited, &proposals).is_err());

    let mut deleted = history;
    deleted.remove(0);
    assert!(validate_legacy_disposition_shard(&deleted, &proposals).is_err());
}

/// A landing batch and the aggregate tolerate the same legacy rows.
///
/// The landing path used to run `validate_proposal_intrinsic` over every
/// incoming proposal, and that rule refuses any row not written `proposed`.
/// The aggregate runs it over live rows only and judges terminal ones by the
/// shipped fingerprint instead, so the shipped rows were admitted by one
/// reader and refused by the other. Both now read a terminal row the same way.
/// The pairs below are the shipped set and the shipped set with one byte
/// changed, and the assertion is that the two paths reach the same verdict on
/// each - not merely that the landing path is more permissive than it was.
#[test]
fn landing_tolerates_the_terminal_rows_the_aggregate_tolerates() {
    for forged in [false, true] {
        let (_dir, store, scope) = super::super::initialized_store();
        let mut proposals = shipped_terminal_proposals();
        if forged {
            proposals[0].summary.push('x');
        }
        let dispositions = shipped_audit();

        let by_aggregate =
            provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                disposition_actor_ids: &store.manifest().unwrap().disposition_actor_ids,
                contributions: &[],
                synthesis_packets: &[],
                proposals: &proposals,
                assertions: &[],
                dispositions: &dispositions,
            });
        let by_landing = store.land_ideation_batch(
            &scope,
            crate::state_store::IdeationLandingBatch {
                contributions: Vec::new(),
                synthesis_packets: Vec::new(),
                proposals: proposals.clone(),
                assertions: Vec::new(),
                dispositions: dispositions.clone(),
            },
            false,
        );

        assert_eq!(
            by_landing.is_ok(),
            by_aggregate.is_ok(),
            "forged={forged}: landing said {by_landing:?}, the aggregate said {by_aggregate:?}"
        );
        assert_eq!(
            by_landing.is_ok(),
            !forged,
            "forged={forged}: {by_landing:?}"
        );
        assert_eq!(
            store.list_proposal_definitions(&scope).unwrap().len(),
            if forged { 0 } else { proposals.len() }
        );
    }
}

/// The audit as it shipped, read from this repository's retired shard - the
/// one set the frozen fingerprint in `provenance-core` was taken over.
fn shipped_audit() -> Vec<DispositionRecord> {
    let rows = shipped_ideation_shard::<DispositionRecord>("promotion_decisions.jsonl");
    assert!(!rows.is_empty(), "the shipped audit is not empty");
    rows
}

/// The terminal proposal rows as they shipped, which is the other half of the
/// same frozen history.
fn shipped_terminal_proposals() -> Vec<ProposalCard> {
    let rows = shipped_ideation_shard::<ProposalCard>("proposal_cards.jsonl")
        .into_iter()
        .filter(|proposal| proposal.promotion_state != PromotionState::Proposed)
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "the shipped terminal set is not empty");
    rows
}

fn shipped_ideation_shard<T: serde::de::DeserializeOwned>(file: &str) -> Vec<T> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".provenance/state/scopes/default/ideation")
        .join(file);
    let shard = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("shipped shard at {}: {error}", path.display()));
    shard
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// One proposal per row, all in `state`, so the promotion-state axis moves on
/// its own.
fn proposals_for(rows: &[DispositionRecord], state: PromotionState) -> Vec<ProposalCard> {
    rows.iter()
        .map(|row| row.proposal_id.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| proposal(id, state))
        .collect()
}

fn proposal(id: &str, promotion_state: PromotionState) -> ProposalCard {
    ProposalCard {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        proposal_key: "overtime".into(),
        proposal_type: ProposalType::RequirementCandidate,
        title: "Overtime".into(),
        summary: "Clarify the overtime requirement.".into(),
        confidence: None,
        traceability: ProposalTraceability {
            target: IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: StableId::new("req_overtime").unwrap(),
            },
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            supporting_claim_ids: Vec::new(),
        },
        builds_on: Vec::new(),
        promotion_state,
        duplicate_of: None,
        superseded_by: None,
    }
}

fn legacy_row(id: &str, proposal_id: &str) -> DispositionRecord {
    DispositionRecord {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        proposal_id: StableId::new(proposal_id).unwrap(),
        decision: DispositionDecision::Accepted,
        rationale: "Accepted by the shipped review panel.".into(),
        actor: DispositionActor {
            identity_type: IdentityType::Human,
            id: "reviewer".into(),
            name: None,
        },
        canonical_artifact: None,
        external_action: None,
    }
}
