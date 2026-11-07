use super::{
    assertion_validation, effective_proposal_state, legacy_validation, lineage_validation,
    validate_proposal_intrinsic, AssertionRecord, IdeationAggregate,
};
use crate::model::{
    disposition_requires_prior_assertion, validate_disposition_intrinsic, DispositionActorRefusal,
    DispositionRecord, PromotionState, ProposalCard,
};
use provenance_macros::rule;
use serde::Serialize;
use std::collections::BTreeMap;

pub(super) fn validate(aggregate: IdeationAggregate<'_>) -> anyhow::Result<()> {
    ensure_immutable_ids(
        "contribution",
        aggregate
            .contributions
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    ensure_immutable_ids(
        "synthesis packet",
        aggregate
            .synthesis_packets
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    ensure_immutable_ids(
        "proposal",
        aggregate
            .proposals
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    legacy_validation::validate_records(
        aggregate.proposals,
        aggregate.dispositions,
        aggregate.legacy_policy,
    )?;
    validate_proposals(aggregate.proposals)?;
    ensure_immutable_ids(
        "assertion",
        aggregate
            .assertions
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    ensure_immutable_ids(
        "disposition",
        aggregate
            .dispositions
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    validate_disposition_intrinsics(aggregate.dispositions)?;

    let proposals = aggregate
        .proposals
        .iter()
        .map(|proposal| (proposal.id.as_str(), proposal))
        .collect::<BTreeMap<_, _>>();
    validate_actor_allowlists(&aggregate, &proposals)?;
    lineage_validation::validate(aggregate.proposals, aggregate.assertions)?;
    let synthesis_packets = aggregate
        .synthesis_packets
        .iter()
        .map(|packet| (packet.id.as_str(), packet))
        .collect::<BTreeMap<_, _>>();
    assertion_validation::validate_assertions(&aggregate, &proposals, &synthesis_packets)?;
    validate_disposition_links(&aggregate)?;
    assertion_validation::ensure_qualifying_assertions(
        aggregate.proposals,
        aggregate.synthesis_packets,
        aggregate.assertions,
        aggregate.dispositions,
    )
}

fn validate_proposals(proposals: &[ProposalCard]) -> anyhow::Result<()> {
    for proposal in proposals {
        if proposal.promotion_state == PromotionState::Proposed {
            validate_proposal_intrinsic(proposal)?;
        }
    }
    Ok(())
}

fn validate_disposition_intrinsics(dispositions: &[DispositionRecord]) -> anyhow::Result<()> {
    for disposition in dispositions {
        validate_disposition_intrinsic(disposition)?;
    }
    Ok(())
}

/// Who may dispose of a live proposal.
///
/// A disposition ends a proposal's life, so only an actor named in the
/// repository's manifest may record one. The check bites on a proposal whose
/// effective pre-disposition state is live (`proposed` or `asserted`); a legacy
/// terminal row is frozen by its shipped fingerprint and carries its own audit.
///
/// The allowlist is manifest state, written at `provenance init` and read
/// on every write path that validates the aggregate. An empty list therefore
/// blocks every disposition, which is the safe default for a repository that
/// has not yet said who decides.
///
/// The actor id is an attestation, not proof of identity. Nothing here checks
/// a key or a signature: the id records who claims to have decided, and write
/// access to the repository is the only gate behind that claim.
fn validate_actor_allowlists(
    aggregate: &IdeationAggregate<'_>,
    proposals: &BTreeMap<&str, &ProposalCard>,
) -> anyhow::Result<()> {
    for disposition in aggregate.dispositions {
        if let Some(proposal) = proposals.get(disposition.proposal_id.as_str()) {
            validate_actor_allowlist(
                proposal,
                aggregate.assertions,
                &disposition.actor.id,
                aggregate.disposition_actor_ids,
            )?;
        }
    }
    Ok(())
}

#[rule("rule_disposition_actor_allowlist")]
fn validate_actor_allowlist(
    proposal: &ProposalCard,
    assertions: &[AssertionRecord],
    actor_id: &str,
    disposition_actor_ids: &[String],
) -> anyhow::Result<()> {
    let requires_allowlisted_actor = matches!(
        effective_proposal_state(proposal, assertions, &[]),
        PromotionState::Proposed | PromotionState::Asserted
    );
    if requires_allowlisted_actor && !disposition_actor_ids.iter().any(|id| id == actor_id) {
        return Err(allowlist_refusal(disposition_actor_ids).into());
    }
    Ok(())
}

const fn allowlist_refusal(disposition_actor_ids: &[String]) -> DispositionActorRefusal {
    if disposition_actor_ids.is_empty() {
        DispositionActorRefusal::NoActorsConfigured
    } else {
        DispositionActorRefusal::ActorNotListed
    }
}

/// What every disposition must satisfy, wherever it is judged.
///
/// A disposition is the record that ends a proposal's life, and two readers
/// judge one: this aggregate, which reads a whole scope, and
/// `provenance-store`'s disposition write gate, which reads a single
/// disposition against what the scope already holds. Three of the tests are
/// the same test in both, so they are stated once here and both callers ask
/// them:
///
/// 1. the proposal named exists, so no decision lands on nothing;
/// 2. an acceptance names a proposal that already carries an assertion, unless
///    [`disposition_requires_prior_assertion`] says a person's ratified
///    artifact stands where the assertion would. A proposal that has already
///    left `proposed` is shipped history, frozen by its own fingerprint and
///    carrying its own audit, so it is not asked again;
/// 3. nothing has ended this proposal already and the disposition id is still
///    free, so a proposal is disposed of once and no decision overwrites
///    another.
///
/// `recorded` is the dispositions the caller already holds: every disposition
/// in the scope for the write gate, and the ones already walked for the
/// aggregate. The proposal found by the first test is returned, so a caller
/// that needs the record does not look it up twice.
///
/// One test the write gate makes is not here: that the canonical artifact a
/// disposition names exists in the scope under the kind it claims. Only the
/// store can see artifacts.
pub fn validate_disposition_admissible<'a>(
    disposition: &DispositionRecord,
    proposals: &'a [ProposalCard],
    assertions: &[AssertionRecord],
    recorded: &[DispositionRecord],
) -> anyhow::Result<&'a ProposalCard> {
    let proposal = proposals
        .iter()
        .find(|proposal| proposal.id == disposition.proposal_id)
        .ok_or_else(|| anyhow::anyhow!("proposal does not exist"))?;
    if proposal.promotion_state == PromotionState::Proposed
        && disposition_requires_prior_assertion(disposition)
        && !assertions
            .iter()
            .any(|assertion| assertion.proposal_id == disposition.proposal_id)
    {
        return Err(crate::model::ApprovalEvidenceRequired.into());
    }
    anyhow::ensure!(
        !recorded
            .iter()
            .any(|record| record.id == disposition.id
                || record.proposal_id == disposition.proposal_id),
        "proposal already has an authoritative disposition"
    );
    Ok(proposal)
}

/// Which proposal each disposition ends, and what had to be recorded first.
///
/// [`validate_disposition_admissible`] holds everything this shares with
/// `provenance-store`'s write gate. What is left is what only a reader of
/// whole records can ask: that a disposition sits in
/// the scope its proposal sits in.
fn validate_disposition_links(aggregate: &IdeationAggregate<'_>) -> anyhow::Result<()> {
    for (index, disposition) in aggregate.dispositions.iter().enumerate() {
        let proposal = validate_disposition_admissible(
            disposition,
            aggregate.proposals,
            aggregate.assertions,
            &aggregate.dispositions[..index],
        )?;
        anyhow::ensure!(
            disposition.scope_id == proposal.scope_id,
            "disposition must share the proposal scope"
        );
    }
    Ok(())
}

fn ensure_immutable_ids<'a, T: Serialize + 'a>(
    kind: &str,
    records: impl IntoIterator<Item = (&'a str, &'a T)>,
) -> anyhow::Result<()> {
    let mut seen = BTreeMap::new();
    for (id, record) in records {
        let value = serde_json::to_value(record)?;
        anyhow::ensure!(
            seen.insert(id, value).is_none(),
            "duplicate immutable {kind} id {id}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod disposition_actor_trigger_tests {
    use provenance_macros::verifies;
    use serde_json::json;

    use super::validate_actor_allowlist;
    use crate::model::{AssertionRecord, PromotionState, ProposalCard};

    #[test]
    #[verifies("rule_disposition_actor_allowlist", examples)]
    fn trigger_uses_the_proposals_derived_effective_state() {
        let mut proposal: ProposalCard = serde_json::from_value(json!({
            "scope_id": "default", "id": "proposal_a",
            "proposal_key": "proposal_a", "proposal_type": "question",
            "title": "Proposal", "summary": "Proposal",
            "traceability": {
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
            },
            "promotion_state": "proposed"
        }))
        .unwrap();
        let assertion: AssertionRecord = serde_json::from_value(json!({
            "scope_id": "default", "id": "assertion_a",
            "proposal_id": "proposal_a", "synthesis_packet_id": "synthesis_a",
            "supporting_claim_ids": ["claim_a"]
        }))
        .unwrap();

        let error = validate_actor_allowlist(&proposal, &[assertion], "reviewer", &[])
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("no disposition actors configured"));

        proposal.promotion_state = PromotionState::Accepted;
        validate_actor_allowlist(&proposal, &[], "reviewer", &[]).unwrap();
    }
}
