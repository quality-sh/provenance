use crate::handlers::check::index::CheckIndex;
use crate::handlers::check::references::{check_ideation_target, check_scoped_reference};
use crate::store::ScopeSnapshot;
use provenance_core::{Contribution, DispositionRecord, ProposalCard, ScopeId, SynthesisPacket};

pub(super) struct Records<'a> {
    contributions: &'a [Contribution],
    synthesis_packets: &'a [SynthesisPacket],
    proposal_cards: &'a [ProposalCard],
    dispositions: &'a [DispositionRecord],
}

impl<'a> Records<'a> {
    pub(super) fn load(snapshot: &'a ScopeSnapshot) -> Self {
        Self {
            contributions: &snapshot.contributions,
            synthesis_packets: &snapshot.synthesis_packets,
            proposal_cards: &snapshot.proposal_cards,
            dispositions: &snapshot.dispositions,
        }
    }

    pub(super) fn validate_scope_ownership(
        &self,
        loaded_scope_id: &ScopeId,
        findings: &mut Vec<String>,
    ) {
        macro_rules! check_records {
            ($records:expr, $record_type:literal) => {
                for record in $records {
                    super::check_scope_ownership(
                        loaded_scope_id,
                        &record.scope_id,
                        $record_type,
                        &record.id,
                        findings,
                    );
                }
            };
        }

        check_records!(self.contributions, "contribution");
        check_records!(self.synthesis_packets, "synthesis packet");
        check_records!(self.proposal_cards, "proposal");
        check_records!(self.dispositions, "disposition");
    }

    pub(super) fn add_to(&self, index: &mut CheckIndex) {
        for proposal in self.proposal_cards {
            index.add_node(&proposal.scope_id, "proposal", &proposal.id);
        }
    }

    pub(super) fn validate(
        &self,
        index: &CheckIndex,
        scope_id: &ScopeId,
        dangling: &mut Vec<String>,
    ) {
        for contribution in self.contributions {
            check_ideation_target(
                index,
                dangling,
                scope_id,
                &format!("contribution {}", contribution.id.as_str()),
                &contribution.target,
            );
        }
        for synthesis_packet in self.synthesis_packets {
            check_ideation_target(
                index,
                dangling,
                scope_id,
                &format!("synthesis packet {}", synthesis_packet.id.as_str()),
                &synthesis_packet.target,
            );
        }
        for proposal in self.proposal_cards {
            let owner = format!("proposal {}", proposal.id.as_str());
            check_ideation_target(
                index,
                dangling,
                scope_id,
                &owner,
                &proposal.traceability.target,
            );
            for source_id in &proposal.traceability.source_ids {
                check_scoped_reference(
                    index,
                    dangling,
                    scope_id,
                    &owner,
                    "source_id",
                    "source",
                    source_id,
                );
            }
            if let Some(duplicate_of) = &proposal.duplicate_of {
                check_scoped_reference(
                    index,
                    dangling,
                    scope_id,
                    &owner,
                    "duplicate_of",
                    "proposal",
                    duplicate_of,
                );
            }
            if let Some(superseded_by) = &proposal.superseded_by {
                check_scoped_reference(
                    index,
                    dangling,
                    scope_id,
                    &owner,
                    "superseded_by",
                    "proposal",
                    superseded_by,
                );
            }
        }
        for disposition in self.dispositions {
            check_scoped_reference(
                index,
                dangling,
                scope_id,
                &format!("disposition {}", disposition.id.as_str()),
                "proposal",
                "proposal",
                &disposition.proposal_id,
            );
        }
    }
}
