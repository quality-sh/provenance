use super::canonical_artifacts::CanonicalArtifactIndex;
use super::{
    serde_name, CreateAssertionInput, CreateDispositionInput, CreateProposalCardInput,
    MutationAuth, StateStore,
};
use crate::shards;
use provenance_core::{
    validate_optional_confidence_score, AssertionRecord, Capability, DispositionRecord,
    PromotionState, ProposalCard, RbacClaim, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;

impl StateStore {
    pub fn create_asserted_proposal(
        &self,
        claim: Option<&RbacClaim>,
        proposal: CreateProposalCardInput,
        assertion: CreateAssertionInput,
    ) -> anyhow::Result<ProposalCard> {
        anyhow::ensure!(
            proposal.scope_id == assertion.scope_id,
            "proposal and assertion scope_id must match"
        );
        anyhow::ensure!(
            proposal.id == assertion.proposal_id,
            "assertion proposal_id must match proposal id"
        );
        let scope = proposal.scope_id.clone();
        self.with_lifecycle_lock(&scope, || {
            let proposal = proposal_from_input(proposal)?;
            let assertion = AssertionRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: assertion.scope_id,
                id: assertion.id,
                proposal_id: assertion.proposal_id,
                synthesis_packet_id: assertion.synthesis_packet_id,
                supporting_claim_ids: assertion.supporting_claim_ids,
            };
            self.write_ideation_batch(
                claim,
                &scope,
                super::IdeationLandingBatch {
                    contributions: Vec::new(),
                    synthesis_packets: Vec::new(),
                    proposals: vec![proposal.clone()],
                    assertions: vec![assertion],
                    dispositions: Vec::new(),
                },
                false,
            )?;
            Ok(proposal)
        })
    }

    pub fn assert_proposal(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateAssertionInput,
    ) -> anyhow::Result<AssertionRecord> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_assertion(claim, input))
    }

    pub fn assert_proposal_after_human_decision(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateAssertionInput,
        decision_keys: &[StableId],
    ) -> anyhow::Result<AssertionRecord> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || {
            self.ensure_proposal_not_disposed(&scope, &input.proposal_id)?;
            let mut synthesis = self
                .list_synthesis_packets(&scope)?
                .into_iter()
                .find(|packet| packet.id == input.synthesis_packet_id)
                .ok_or_else(|| anyhow::anyhow!("assertion synthesis packet does not exist"))?;
            let decision_keys = decision_keys
                .iter()
                .map(StableId::as_str)
                .collect::<std::collections::BTreeSet<_>>();
            anyhow::ensure!(
                !decision_keys.is_empty(),
                "no human decision key was supplied"
            );
            let resolved = synthesis
                .required_human_decisions
                .iter()
                .filter(|decision| {
                    decision.blocks_promotion
                        && decision_keys.contains(decision.decision_key.as_str())
                })
                .count();
            anyhow::ensure!(
                resolved == decision_keys.len(),
                "synthesis packet has no matching blocking human decision to resolve"
            );
            synthesis.required_human_decisions.retain(|decision| {
                !decision.blocks_promotion
                    || !decision_keys.contains(decision.decision_key.as_str())
            });
            let assertion = AssertionRecord {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: input.scope_id,
                id: input.id,
                proposal_id: input.proposal_id,
                synthesis_packet_id: input.synthesis_packet_id,
                supporting_claim_ids: input.supporting_claim_ids,
            };
            self.write_ideation_batch(
                claim,
                &scope,
                super::IdeationLandingBatch {
                    contributions: Vec::new(),
                    synthesis_packets: vec![synthesis],
                    proposals: Vec::new(),
                    assertions: vec![assertion.clone()],
                    dispositions: Vec::new(),
                },
                true,
            )?;
            Ok(assertion)
        })
    }

    /// A disposition ends a proposal's life, so nothing may assert it
    /// afterwards. Both assertion entry points ask this, which keeps the
    /// refusal and its wording in one place.
    fn ensure_proposal_not_disposed(
        &self,
        scope_id: &ScopeId,
        proposal_id: &StableId,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self
                .list_dispositions(scope_id)?
                .iter()
                .any(|record| record.proposal_id == *proposal_id),
            "a disposed proposal cannot re-enter assertion"
        );
        Ok(())
    }

    fn write_assertion(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateAssertionInput,
    ) -> anyhow::Result<AssertionRecord> {
        let assertion = AssertionRecord {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: input.scope_id.clone(),
            id: input.id,
            proposal_id: input.proposal_id,
            synthesis_packet_id: input.synthesis_packet_id,
            supporting_claim_ids: input.supporting_claim_ids,
        };
        self.ensure_proposal_not_disposed(&input.scope_id, &assertion.proposal_id)?;
        let path = shards::assertion_records_path(&self.layout, &input.scope_id);
        let auth = MutationAuth::new(claim, Capability::Execute, &input.scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<AssertionRecord>| {
            anyhow::ensure!(
                !records.iter().any(|record| {
                    record.id == assertion.id || record.proposal_id == assertion.proposal_id
                }),
                "assertion identity or proposal assertion already exists"
            );
            let mut assertions = self.list_assertion_records(&input.scope_id)?;
            assertions.push(assertion.clone());
            let manifest = self.manifest()?;
            provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                ratification: manifest.disposition_ratification(),
                contributions: &self.list_contributions(&input.scope_id)?,
                synthesis_packets: &self.list_synthesis_packets(&input.scope_id)?,
                proposals: &self.list_proposal_definitions(&input.scope_id)?,
                assertions: &assertions,
                dispositions: &self.list_dispositions(&input.scope_id)?,
            })?;
            records.push(assertion.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(assertion)
        })
    }

    pub fn create_proposal_card(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateProposalCardInput,
    ) -> anyhow::Result<ProposalCard> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_proposal_card(claim, input))
    }

    fn write_proposal_card(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateProposalCardInput,
    ) -> anyhow::Result<ProposalCard> {
        let candidate = proposal_from_input(input)?;
        let mut proposals = self.list_proposal_definitions(&candidate.scope_id)?;
        proposals.push(candidate.clone());
        let manifest = self.manifest()?;
        provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
            legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
            ratification: manifest.disposition_ratification(),
            contributions: &self.list_contributions(&candidate.scope_id)?,
            synthesis_packets: &self.list_synthesis_packets(&candidate.scope_id)?,
            proposals: &proposals,
            assertions: &self.list_assertion_records(&candidate.scope_id)?,
            dispositions: &self.list_dispositions(&candidate.scope_id)?,
        })?;
        let path = shards::proposal_cards_path(&self.layout, &candidate.scope_id);
        let auth = MutationAuth::new(claim, Capability::Execute, &candidate.scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<ProposalCard>| {
            if let Some(index) = records.iter().position(|record| record.id == candidate.id) {
                anyhow::bail!(
                    "proposal {} already exists and is immutable",
                    records[index].id.as_str()
                );
            }
            records.push(candidate.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(candidate)
        })
    }

    pub fn create_disposition(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateDispositionInput,
    ) -> anyhow::Result<DispositionRecord> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_disposition(claim, input))
    }

    fn write_disposition(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateDispositionInput,
    ) -> anyhow::Result<DispositionRecord> {
        let CreateDispositionInput {
            scope_id,
            id,
            proposal_id,
            decision,
            rationale,
            actor,
            canonical_artifact,
            external_action,
        } = input;
        let disposition = DispositionRecord {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope_id.clone(),
            id,
            proposal_id,
            decision,
            rationale,
            actor,
            canonical_artifact,
            external_action,
        };
        let proposals = self.list_proposal_definitions(&scope_id)?;
        let assertions = self.list_assertion_records(&scope_id)?;
        let mut dispositions = self.list_dispositions(&scope_id)?;
        validate_disposition_write_gate(
            &disposition,
            &proposals,
            &assertions,
            &dispositions,
            &self.canonical_artifact_index(&scope_id)?,
        )?;
        dispositions.push(disposition.clone());
        let manifest = self.manifest()?;
        provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
            legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
            ratification: manifest.disposition_ratification(),
            contributions: &self.list_contributions(&scope_id)?,
            synthesis_packets: &self.list_synthesis_packets(&scope_id)?,
            proposals: &proposals,
            assertions: &assertions,
            dispositions: &dispositions,
        })?;
        let dispositions_path = shards::dispositions_path(&self.layout, &scope_id);
        let auth = MutationAuth::new(claim, Capability::Execute, &scope_id);
        self.mutate_jsonl_records(
            &dispositions_path,
            auth,
            |records: &mut Vec<DispositionRecord>| {
                anyhow::ensure!(
                    !records.iter().any(|record| record.id == disposition.id),
                    "disposition already exists"
                );
                records.push(disposition.clone());
                records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
                Ok(disposition)
            },
        )
    }

    pub fn ensure_proposal_card_replaceable(
        &self,
        scope_id: &ScopeId,
        proposal_id: &StableId,
    ) -> anyhow::Result<()> {
        if let Some(existing) = self
            .list_proposal_definitions(scope_id)?
            .into_iter()
            .find(|proposal| proposal.id.as_str() == proposal_id.as_str())
        {
            self.ensure_existing_proposal_card_replaceable(scope_id, &existing)?;
        }
        Ok(())
    }

    fn ensure_existing_proposal_card_replaceable(
        &self,
        scope_id: &ScopeId,
        existing: &ProposalCard,
    ) -> anyhow::Result<()> {
        let decisions = self
            .list_dispositions(scope_id)?
            .into_iter()
            .filter(|decision| decision.proposal_id.as_str() == existing.id.as_str())
            .collect::<Vec<_>>();
        if existing.promotion_state == PromotionState::Proposed && decisions.is_empty() {
            return Ok(());
        }

        let state = serde_name(&existing.promotion_state)
            .unwrap_or_else(|_| format!("{:?}", existing.promotion_state));
        if decisions.is_empty() {
            anyhow::bail!(
                "proposal {} has a human disposition and cannot be replaced; promotion_state is {state}",
                existing.id.as_str()
            );
        }

        let decision_ids = decisions
            .iter()
            .map(|decision| decision.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!(
            "proposal {} has a human disposition and cannot be replaced; promotion_state is {state}; disposition(s): {decision_ids}",
            existing.id.as_str()
        );
    }
}

/// What a person's disposition must satisfy before it is written.
///
/// A disposition is the record that ends a proposal's life, so the write is
/// refused unless all four hold:
///
/// 1. the proposal named exists in the scope, so no decision lands on nothing;
/// 2. an `accepted` decision on a live proposal names one that already carries
///    an assertion (`rejected` and `deferred` need no assertion, because
///    refusing a proposal is not a claim about its evidence; nor does an
///    acceptance naming a canonical artifact, because that ratified artifact
///    is the evidence; nor a row that already left `proposed`, which is legacy
///    history carrying its own audit);
/// 3. the proposal has no disposition yet and the disposition id is free, so
///    a person disposes of a proposal exactly once;
/// 4. the canonical artifact named, when one is named, exists in the same
///    scope under the kind it claims.
///
/// The first three are [`provenance_core::validate_disposition_admissible`],
/// which the ideation aggregate also runs, so this gate and that aggregate
/// refuse the same writes in the same words. The fourth is this gate's alone:
/// only the store can see the scope's artifacts.
///
/// The gate reads state the caller has already loaded and writes nothing; the
/// caller holds the scope's lifecycle lock across both, so the state it judges
/// is the state it is judged against. The checks run in the order above, so a
/// caller who breaks several hears about the first.
#[rule("rule_disposition_write_gate")]
pub(super) fn validate_disposition_write_gate(
    disposition: &DispositionRecord,
    proposals: &[ProposalCard],
    assertions: &[AssertionRecord],
    dispositions: &[DispositionRecord],
    canonical_artifacts: &CanonicalArtifactIndex,
) -> anyhow::Result<()> {
    provenance_core::validate_disposition_admissible(
        disposition,
        proposals,
        assertions,
        dispositions,
    )?;
    canonical_artifacts.ensure_exists(disposition.canonical_artifact.as_ref())
}

fn proposal_from_input(input: CreateProposalCardInput) -> anyhow::Result<ProposalCard> {
    match input.promotion_state {
        PromotionState::Duplicate => {
            anyhow::ensure!(
                input.duplicate_of.is_some(),
                "duplicate proposals must set duplicate_of"
            );
        }
        PromotionState::Superseded => {
            anyhow::ensure!(
                input.superseded_by.is_some(),
                "superseded proposals must set superseded_by"
            );
        }
        PromotionState::Proposed
        | PromotionState::Asserted
        | PromotionState::Accepted
        | PromotionState::Rejected
        | PromotionState::Deferred => {}
    }
    let proposal = ProposalCard {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: input.scope_id,
        id: input.id,
        proposal_key: input.proposal_key,
        proposal_type: input.proposal_type,
        title: input.title,
        summary: input.summary,
        confidence: validate_optional_confidence_score(input.confidence)?,
        traceability: input.traceability,
        promotion_state: input.promotion_state,
        builds_on: input.builds_on,
        duplicate_of: input.duplicate_of,
        superseded_by: input.superseded_by,
    };
    provenance_core::validate_proposal_intrinsic(&proposal)?;
    Ok(proposal)
}
