//! The scope export document: every record of one scope, as export writes it and
//! import reads it.

use crate::snapshot::ScopeSnapshot;
use serde::Serialize;

/// Every record of one scope.
#[derive(Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeExport {
    pub scope: String,
    pub sources: Vec<provenance_core::Source>,
    #[serde(default)]
    pub domains: Vec<provenance_core::Domain>,
    pub requirements: Vec<provenance_core::Requirement>,
    #[serde(default)]
    pub boundaries: Vec<provenance_core::Boundary>,
    #[serde(default)]
    pub topics: Vec<provenance_core::Topic>,
    #[serde(default)]
    pub questions: Vec<provenance_core::Question>,
    pub resolutions: Vec<provenance_core::Resolution>,
    pub rules: Vec<provenance_core::Rule>,
    #[serde(default)]
    pub verification_bindings: Vec<provenance_core::VerificationBinding>,
    #[serde(default)]
    pub implementation_bindings: Vec<provenance_core::ImplementationBinding>,
    pub threads: Vec<provenance_core::Thread>,
    pub messages: Vec<provenance_core::Message>,
    #[serde(default)]
    pub contributions: Vec<provenance_core::Contribution>,
    #[serde(default)]
    pub synthesis_packets: Vec<provenance_core::SynthesisPacket>,
    #[serde(default)]
    pub proposal_cards: Vec<provenance_core::ProposalCard>,
    #[serde(default)]
    pub assertion_records: Vec<provenance_core::AssertionRecord>,
    #[serde(default, alias = "promotion_decisions")]
    pub dispositions: Vec<provenance_core::DispositionRecord>,
}

impl ScopeExport {
    /// The export of the records of one scope snapshot.
    pub fn from_snapshot(scope: String, snapshot: ScopeSnapshot) -> Self {
        Self {
            scope,
            sources: snapshot.sources,
            domains: snapshot.domains,
            requirements: snapshot.requirements,
            boundaries: snapshot.boundaries,
            topics: snapshot.topics,
            questions: snapshot.questions,
            resolutions: snapshot.resolutions,
            rules: snapshot.rules,
            verification_bindings: snapshot.verification_bindings,
            implementation_bindings: snapshot.implementation_bindings,
            threads: snapshot.threads,
            messages: snapshot.messages,
            contributions: snapshot.contributions,
            synthesis_packets: snapshot.synthesis_packets,
            proposal_cards: snapshot.proposal_cards,
            assertion_records: snapshot.assertion_records,
            dispositions: snapshot.dispositions,
        }
    }
}
