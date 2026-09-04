use super::aggregate_validation::{ensure_supported_schema_version, ASSERTION_KIND};
use super::{validate_assertion_intrinsic, AssertionRecord, IdeationAggregate};
use crate::model::{
    disposition_requires_prior_assertion, Contribution, DispositionRecord, IdeationEvidenceType,
    MaterialClaim, PromotionState, ProposalCard, StableId, SynthesisPacket,
};
use provenance_macros::rule;
use std::collections::{BTreeMap, BTreeSet};

mod qualification;

pub use qualification::packet_qualifies_proposal;
use qualification::{
    blocking_evidence_gap, blocking_human_decision, packet_adjudicates_proposal,
    packet_owns_proposal_target, qualify_proposal_for_assertion, QualificationFacts,
};

pub(super) fn validate_assertions(
    aggregate: &IdeationAggregate<'_>,
    proposals: &BTreeMap<&str, &ProposalCard>,
    synthesis_packets: &BTreeMap<&str, &SynthesisPacket>,
) -> anyhow::Result<()> {
    let mut asserted_proposals = BTreeSet::new();
    for assertion in aggregate.assertions {
        ensure_supported_schema_version(ASSERTION_KIND, assertion.schema_version)?;
        validate_assertion_intrinsic(assertion)?;
        let proposal = proposals
            .get(assertion.proposal_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("asserted proposal does not exist"))?;
        validate_assertion_proposal(assertion, proposal, &mut asserted_proposals)?;
        let packet = synthesis_packets
            .get(assertion.synthesis_packet_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("assertion synthesis packet does not exist"))?;
        validate_assertion_packet(proposal, packet)?;
        validate_supporting_claims(assertion, proposal, packet, aggregate.contributions)?;
        let assertion_claims = assertion
            .supporting_claim_ids
            .iter()
            .map(StableId::as_str)
            .collect::<BTreeSet<_>>();
        let proposal_claims = proposal
            .traceability
            .supporting_claim_ids
            .iter()
            .map(StableId::as_str)
            .collect::<BTreeSet<_>>();
        anyhow::ensure!(
            assertion_claims == proposal_claims,
            "assertion claims must match proposal traceability"
        );
    }
    Ok(())
}

fn validate_assertion_proposal<'a>(
    assertion: &'a AssertionRecord,
    proposal: &ProposalCard,
    asserted_proposals: &mut BTreeSet<&'a str>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        proposal.promotion_state == PromotionState::Proposed,
        "legacy terminal proposal {} is frozen against lifecycle re-entry",
        proposal.id.as_str()
    );
    anyhow::ensure!(
        assertion.scope_id == proposal.scope_id,
        "assertion must share the proposal scope"
    );
    anyhow::ensure!(
        asserted_proposals.insert(assertion.proposal_id.as_str()),
        "proposal {} has multiple assertions",
        assertion.proposal_id.as_str()
    );
    Ok(())
}

/// The may-assert reading of `qualify_proposal_for_assertion`: an assertion is
/// accepted only against a packet that qualifies its proposal.
///
/// The assertion under validation was looked up through this proposal and this
/// packet, so it already witnesses three of the facts that the must-assert
/// reading has to establish for itself. The packet asserts this proposal and no
/// other, by construction. The proposal cites supporting claims, because
/// `validate_assertion_intrinsic` rejects an assertion with none and
/// `validate_assertions` requires the assertion's claims to match the
/// proposal's. None of those claims is contested, because
/// `validate_supporting_claims` rejects a contested claim and names it. Those
/// three arrive as data; the rest is read off the packet.
fn validate_assertion_packet(
    proposal: &ProposalCard,
    packet: &SynthesisPacket,
) -> anyhow::Result<()> {
    let facts = QualificationFacts {
        packet_owns_proposal_target: packet_owns_proposal_target(packet, proposal),
        packet_adjudicates_proposal: packet_adjudicates_proposal(packet, proposal),
        blocking_evidence_gap: blocking_evidence_gap(packet),
        blocking_human_decision: blocking_human_decision(packet),
        packet_asserts_another_proposal: false,
        proposal_has_supporting_claims: true,
        proposal_claim_contested: false,
    };
    qualify_proposal_for_assertion(facts)
        .map_err(|unmet| anyhow::anyhow!(unmet.describe(proposal.id.as_str())))
}

fn validate_supporting_claims(
    assertion: &AssertionRecord,
    proposal: &ProposalCard,
    packet: &SynthesisPacket,
    contributions: &[Contribution],
) -> anyhow::Result<()> {
    let contested = packet
        .contested_claims
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect::<BTreeSet<_>>();
    for claim_id in &assertion.supporting_claim_ids {
        anyhow::ensure!(
            !contested.contains(claim_id.as_str()),
            "assertion claim {} is contested",
            claim_id.as_str()
        );
        let (claim, owner) = find_claim_owner(claim_id, contributions)?;
        validate_claim_owner(claim_id, proposal, claim, owner)?;
        validate_claim_evidence(claim_id, claim, owner, contributions)?;
    }
    Ok(())
}

fn find_claim_owner<'a>(
    claim_id: &StableId,
    contributions: &'a [Contribution],
) -> anyhow::Result<(&'a MaterialClaim, &'a Contribution)> {
    let owners = contributions
        .iter()
        .flat_map(|contribution| {
            contribution
                .material_claims
                .iter()
                .map(move |claim| (claim, contribution))
        })
        .filter(|(claim, _)| claim.claim_id == *claim_id)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        owners.len() == 1,
        "assertion claim {} must have exactly one owner",
        claim_id.as_str()
    );
    Ok(owners[0])
}

fn validate_claim_owner(
    claim_id: &StableId,
    proposal: &ProposalCard,
    claim: &MaterialClaim,
    owner: &Contribution,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        owner.scope_id == proposal.scope_id && owner.target == proposal.traceability.target,
        "assertion claim {} is not owned by the proposal target",
        claim_id.as_str()
    );
    anyhow::ensure!(
        !claim.evidence_reference_ids.is_empty(),
        "assertion claim {} lacks positive evidence",
        claim_id.as_str()
    );
    anyhow::ensure!(
        is_positive_evidence_type(claim.evidence_type),
        "assertion claim {} must use a positive evidence type",
        claim_id.as_str()
    );
    Ok(())
}

fn validate_claim_evidence(
    claim_id: &StableId,
    claim: &MaterialClaim,
    owner: &Contribution,
    contributions: &[Contribution],
) -> anyhow::Result<()> {
    for evidence_id in &claim.evidence_reference_ids {
        let matches = contributions
            .iter()
            .flat_map(|contribution| {
                contribution
                    .evidence_references
                    .iter()
                    .map(move |evidence| (evidence, contribution))
            })
            .filter(|(evidence, _)| evidence.reference_id == *evidence_id)
            .collect::<Vec<_>>();
        anyhow::ensure!(
            matches.len() == 1,
            "assertion evidence {} must have exactly one owner",
            evidence_id.as_str()
        );
        let (evidence, evidence_owner) = matches[0];
        anyhow::ensure!(
            std::ptr::eq(evidence_owner, owner),
            "assertion evidence {} is not owned by claim contribution {}",
            evidence_id.as_str(),
            owner.id.as_str()
        );
        anyhow::ensure!(
            evidence.evidence_type == claim.evidence_type,
            "assertion evidence type does not match claim {}",
            claim_id.as_str()
        );
        anyhow::ensure!(
            is_positive_evidence_type(evidence.evidence_type),
            "assertion evidence {} must use a positive evidence type",
            evidence_id.as_str()
        );
    }
    Ok(())
}

/// Speculation is not evidence. An assertion may rest only on evidence whose
/// type claims real backing, so the two speculative types, `Unsupported` and
/// `Exploratory`, can never stand behind an asserted claim. Both readings of a
/// supporting claim run through here: the claim's own evidence type
/// (`validate_claim_owner`) and the type of each evidence reference it cites
/// (`validate_claim_evidence`).
#[rule("rule_positive_evidence")]
const fn is_positive_evidence_type(evidence_type: IdeationEvidenceType) -> bool {
    !matches!(
        evidence_type,
        IdeationEvidenceType::Unsupported | IdeationEvidenceType::Exploratory
    )
}

/// A proposal a packet qualifies, with no assertion and no decision behind it,
/// is a hole in the run: the swarm was ready to assert it and never did.
///
/// A proposal already disposed of is not that hole. The dispositions decide
/// which ones those are through the one question both assertion gates ask,
/// [`disposition_requires_prior_assertion`]: a decision that needs no prior
/// assertion - a rejection, a deferral, or a person's acceptance naming the
/// artifact they ratified - closes the proposal on its own terms, and the run
/// has nothing left to assert.
pub(super) fn ensure_qualifying_assertions(
    proposals: &[ProposalCard],
    synthesis_packets: &[SynthesisPacket],
    assertions: &[AssertionRecord],
    dispositions: &[DispositionRecord],
) -> anyhow::Result<()> {
    for proposal in proposals {
        let qualifying = synthesis_packets
            .iter()
            .any(|packet| packet_qualifies_proposal(packet, proposal, assertions));
        let asserted = assertions
            .iter()
            .any(|assertion| assertion.proposal_id == proposal.id);
        let closed_without_assertion = dispositions.iter().any(|disposition| {
            disposition.proposal_id == proposal.id
                && !disposition_requires_prior_assertion(disposition)
        });
        anyhow::ensure!(
            !qualifying || asserted || closed_without_assertion,
            "qualifying proposal {} requires an assertion",
            proposal.id.as_str()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use provenance_macros::verifies;

    use super::is_positive_evidence_type;
    use crate::model::{IdeationEvidenceType, SpeculationMarker};

    /// The size of the domain. The chained match below forces a new evidence
    /// type to join the chain, and this count catches a chain that a new
    /// variant cut short, so the exhaustion proofs stay exhaustive.
    const EVIDENCE_TYPE_COUNT: usize = 6;
    const SPECULATION_MARKER_COUNT: usize = 2;

    // The variant lists are derived from exhaustive matches so that adding a
    // variant fails compilation until the new variant joins the chain.
    fn all_evidence_types() -> Vec<IdeationEvidenceType> {
        let mut all = vec![IdeationEvidenceType::Source];
        while let Some(next) = match all.last().unwrap() {
            IdeationEvidenceType::Source => Some(IdeationEvidenceType::Artifact),
            IdeationEvidenceType::Artifact => Some(IdeationEvidenceType::ThreadMessage),
            IdeationEvidenceType::ThreadMessage => Some(IdeationEvidenceType::DomainKnowledge),
            IdeationEvidenceType::DomainKnowledge => Some(IdeationEvidenceType::Unsupported),
            IdeationEvidenceType::Unsupported => Some(IdeationEvidenceType::Exploratory),
            IdeationEvidenceType::Exploratory => None,
        } {
            all.push(next);
        }
        all
    }

    fn all_speculation_markers() -> Vec<SpeculationMarker> {
        let mut all = vec![SpeculationMarker::Unsupported];
        while let Some(next) = match all.last().unwrap() {
            SpeculationMarker::Unsupported => Some(SpeculationMarker::Exploratory),
            SpeculationMarker::Exploratory => None,
        } {
            all.push(next);
        }
        all
    }

    // Independent restatement of the decision, listed rather than matched so
    // the oracle does not repeat the primary implementation's shape: these are the
    // evidence types that name speculation instead of backing. Must not be
    // implemented by calling the primary implementation.
    const SPECULATIVE_EVIDENCE_TYPES: [IdeationEvidenceType; 2] = [
        IdeationEvidenceType::Unsupported,
        IdeationEvidenceType::Exploratory,
    ];

    fn is_speculation_by_oracle(evidence_type: IdeationEvidenceType) -> bool {
        SPECULATIVE_EVIDENCE_TYPES.contains(&evidence_type)
    }

    // `SpeculationMarker` is the parallel vocabulary: it names the same two
    // kinds of speculation for unsupported recommendations and for a synthesis
    // packet's unsupported speculations, and shares no predicate with this
    // rule. This mapping is the shared statement the two vocabularies do have,
    // and the test below holds it to a correspondence in both directions.
    const fn evidence_type_for_marker(marker: SpeculationMarker) -> IdeationEvidenceType {
        match marker {
            SpeculationMarker::Unsupported => IdeationEvidenceType::Unsupported,
            SpeculationMarker::Exploratory => IdeationEvidenceType::Exploratory,
        }
    }

    #[test]
    #[verifies("rule_positive_evidence", exhaustion)]
    fn evidence_type_is_positive_unless_it_names_speculation() {
        let all = all_evidence_types();
        assert_eq!(
            all.len(),
            EVIDENCE_TYPE_COUNT,
            "the evidence type chain is short of the domain"
        );
        for evidence_type in all {
            assert_eq!(
                is_positive_evidence_type(evidence_type),
                !is_speculation_by_oracle(evidence_type),
                "the rule and the decision disagree on {evidence_type:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_positive_evidence", exhaustion)]
    fn speculation_markers_and_non_positive_evidence_types_name_the_same_set() {
        let markers = all_speculation_markers();
        assert_eq!(
            markers.len(),
            SPECULATION_MARKER_COUNT,
            "the speculation marker chain is short of the domain"
        );
        let mut marked = Vec::new();
        for marker in markers {
            let evidence_type = evidence_type_for_marker(marker);
            assert!(
                !is_positive_evidence_type(evidence_type),
                "{marker:?} names {evidence_type:?}, which the rule counts as positive evidence"
            );
            marked.push(evidence_type);
        }
        for evidence_type in all_evidence_types() {
            if !is_positive_evidence_type(evidence_type) {
                assert!(
                    marked.contains(&evidence_type),
                    "{evidence_type:?} is not positive evidence but no speculation marker names it"
                );
            }
        }
    }
}
