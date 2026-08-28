use super::{CreateContributionInput, CreateSynthesisPacketInput, MutationAuth, StateStore};
use crate::shards;
use provenance_core::{
    validate_optional_confidence_score, Capability, Contribution, RbacClaim, SynthesisPacket,
    SUPPORTED_SCHEMA_VERSION,
};

impl StateStore {
    pub fn create_contribution(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateContributionInput,
    ) -> anyhow::Result<Contribution> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_contribution(claim, input, false))
    }

    pub fn upsert_contribution(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateContributionInput,
    ) -> anyhow::Result<Contribution> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_contribution(claim, input, true))
    }

    fn write_contribution(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateContributionInput,
        replace: bool,
    ) -> anyhow::Result<Contribution> {
        let CreateContributionInput {
            scope_id,
            id,
            target,
            participant_slot,
            stance,
            strongest_finding,
            evidence_references,
            material_claims,
            risks,
            objections,
            challenges,
            suggested_artifact_changes,
            unsupported_recommendations,
            uncertainty,
            open_questions,
        } = input;
        for material_claim in &material_claims {
            validate_optional_confidence_score(material_claim.confidence)?;
        }
        let contribution = Contribution {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope_id.clone(),
            id,
            target,
            participant_slot,
            stance,
            strongest_finding,
            evidence_references,
            material_claims,
            risks,
            objections,
            challenges,
            suggested_artifact_changes,
            unsupported_recommendations,
            uncertainty,
            open_questions,
        };
        let landed = self.list_ideation_landings(&scope_id)?.iter().any(|batch| {
            batch
                .contributions
                .iter()
                .any(|record| record.id == contribution.id)
        });
        if landed {
            anyhow::ensure!(replace, "contribution already exists");
            self.write_ideation_batch(
                claim,
                &scope_id,
                super::IdeationLandingBatch {
                    contributions: vec![contribution.clone()],
                    synthesis_packets: Vec::new(),
                    proposals: Vec::new(),
                    assertions: Vec::new(),
                    dispositions: Vec::new(),
                },
                true,
            )?;
            return Ok(contribution);
        }
        let path = shards::contributions_path(&self.layout, &scope_id);
        let auth = MutationAuth::new(claim, Capability::Execute, &scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<Contribution>| {
            if let Some(index) = records
                .iter()
                .position(|record| record.id == contribution.id)
            {
                anyhow::ensure!(replace, "contribution already exists");
                super::ideation_batches::ensure_asserted_contribution_unchanged(
                    &records[index],
                    &contribution,
                    &self.list_assertion_records(&scope_id)?,
                )?;
                records[index] = contribution.clone();
            } else {
                records.push(contribution.clone());
            }
            let mut contributions = self.list_contributions(&scope_id)?;
            if let Some(index) = contributions
                .iter()
                .position(|record| record.id == contribution.id)
            {
                contributions[index] = contribution.clone();
            } else {
                contributions.push(contribution.clone());
            }
            let manifest = self.manifest()?;
            provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                ratification: manifest.disposition_ratification(),
                contributions: &contributions,
                synthesis_packets: &self.list_synthesis_packets(&scope_id)?,
                proposals: &self.list_proposal_definitions(&scope_id)?,
                assertions: &self.list_assertion_records(&scope_id)?,
                dispositions: &self.list_dispositions(&scope_id)?,
            })?;
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(contribution)
        })
    }

    pub fn create_synthesis_packet(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateSynthesisPacketInput,
    ) -> anyhow::Result<SynthesisPacket> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_synthesis_packet(claim, input, false))
    }

    pub fn upsert_synthesis_packet(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateSynthesisPacketInput,
    ) -> anyhow::Result<SynthesisPacket> {
        let scope = input.scope_id.clone();
        self.with_lifecycle_lock(&scope, || self.write_synthesis_packet(claim, input, true))
    }

    fn write_synthesis_packet(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateSynthesisPacketInput,
        replace: bool,
    ) -> anyhow::Result<SynthesisPacket> {
        let CreateSynthesisPacketInput {
            scope_id,
            id,
            target,
            summary,
            consensus,
            contested_claims,
            minority_objections,
            evidence_gaps,
            unsupported_speculation,
            open_questions,
            suggested_artifacts,
            required_human_decisions,
        } = input;
        let synthesis_packet = SynthesisPacket {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope_id.clone(),
            id,
            target,
            summary,
            consensus,
            contested_claims,
            minority_objections,
            evidence_gaps,
            unsupported_speculation,
            open_questions,
            suggested_artifacts,
            required_human_decisions,
        };
        let landed = self.list_ideation_landings(&scope_id)?.iter().any(|batch| {
            batch
                .synthesis_packets
                .iter()
                .any(|record| record.id == synthesis_packet.id)
        });
        if landed {
            anyhow::ensure!(replace, "synthesis packet already exists");
            self.write_ideation_batch(
                claim,
                &scope_id,
                super::IdeationLandingBatch {
                    contributions: Vec::new(),
                    synthesis_packets: vec![synthesis_packet.clone()],
                    proposals: Vec::new(),
                    assertions: Vec::new(),
                    dispositions: Vec::new(),
                },
                true,
            )?;
            return Ok(synthesis_packet);
        }
        let path = shards::synthesis_packets_path(&self.layout, &scope_id);
        let auth = MutationAuth::new(claim, Capability::Execute, &scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<SynthesisPacket>| {
            if let Some(index) = records
                .iter()
                .position(|record| record.id == synthesis_packet.id)
            {
                anyhow::ensure!(replace, "synthesis packet already exists");
                super::ideation_batches::ensure_asserted_synthesis_unchanged(
                    &records[index],
                    &synthesis_packet,
                    &self.list_assertion_records(&scope_id)?,
                )?;
                records[index] = synthesis_packet.clone();
            } else {
                records.push(synthesis_packet.clone());
            }
            let mut synthesis_packets = self.list_synthesis_packets(&scope_id)?;
            if let Some(index) = synthesis_packets
                .iter()
                .position(|record| record.id == synthesis_packet.id)
            {
                synthesis_packets[index] = synthesis_packet.clone();
            } else {
                synthesis_packets.push(synthesis_packet.clone());
            }
            let manifest = self.manifest()?;
            provenance_core::validate_ideation_aggregate(provenance_core::IdeationAggregate {
                legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
                ratification: manifest.disposition_ratification(),
                contributions: &self.list_contributions(&scope_id)?,
                synthesis_packets: &synthesis_packets,
                proposals: &self.list_proposal_definitions(&scope_id)?,
                assertions: &self.list_assertion_records(&scope_id)?,
                dispositions: &self.list_dispositions(&scope_id)?,
            })?;
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(synthesis_packet)
        })
    }
}
