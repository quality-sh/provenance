use crate::wiki::links::EvidenceRef;
use crate::wiki::model::{
    DecisionSection, EvidenceThread, FieldNote, InputCitation, RuleCard, SourceCitation,
};
use provenance_core::{
    Message, NodeType, Requirement, Resolution, ResolutionInput, Rule, Source, StableId, Thread,
};
use std::collections::BTreeSet;

use super::context::Assembler;
use super::page_links::{resolution_link, rule_link, source_link};

impl Assembler<'_> {
    pub(super) fn input_citation(&self, input: &ResolutionInput) -> InputCitation {
        InputCitation {
            input_type: input.input_type.clone(),
            summary: input.summary.clone(),
            reference: self.resolver.resolve(&input.reference),
        }
    }

    pub(super) fn decision_section(&self, resolution: &Resolution) -> DecisionSection {
        DecisionSection {
            link: resolution_link(resolution),
            status: resolution.status.clone(),
            position: resolution.position.clone(),
            rationale: resolution.rationale.clone(),
            context: resolution.context.clone(),
            enforcement: resolution.enforcement.clone(),
            confidence: resolution.confidence,
            inputs: resolution
                .inputs
                .iter()
                .map(|input| self.input_citation(input))
                .collect(),
            made_by: resolution.made_by.clone(),
            approved_by: resolution.approved_by.clone(),
            approved_at: resolution.approved_at,
        }
    }

    pub(super) fn rule_evidence(&self, rule: &Rule) -> Vec<EvidenceRef> {
        rule.source_document
            .as_ref()
            .map(|document| {
                vec![self
                    .resolver
                    .resolve_document(document, rule.source_section.as_deref(), None)]
            })
            .unwrap_or_default()
    }

    pub(super) fn rule_card(&self, rule: &Rule) -> RuleCard {
        RuleCard {
            link: rule_link(rule),
            statement: rule.statement.clone(),
            status: rule.status.clone(),
            severity: rule.severity.clone(),
            evidence: self.rule_evidence(rule),
        }
    }

    pub(super) fn source_reference_link(&self, source: &Source) -> Option<EvidenceRef> {
        source.reference.as_ref().map(|reference| {
            self.resolver
                .resolve_at(reference, source.commit_pin.as_deref())
        })
    }

    pub(super) fn source_citation(
        &self,
        source: &Source,
        clause: Option<String>,
    ) -> SourceCitation {
        SourceCitation {
            link: source_link(source),
            source_type: source.source_type.clone(),
            clause,
            reference: self.source_reference_link(source),
        }
    }

    /// The citations a requirement carries, one per source it cites. Gap
    /// detection for missing and dangling references is shared with prime
    /// via `compute_gaps`.
    pub(super) fn requirement_sources(&self, requirement: &Requirement) -> Vec<SourceCitation> {
        let mut citations = Vec::new();
        let mut cited: BTreeSet<&str> = BTreeSet::new();
        for reference in &requirement.source_refs {
            if let Some(source) = self.find_source(&reference.source_id) {
                if cited.insert(source.id.as_str()) {
                    citations.push(self.source_citation(source, reference.clause.clone()));
                }
            }
        }
        citations
    }

    pub(super) fn evidence_thread(&self, thread: &Thread) -> EvidenceThread {
        let mut messages: Vec<&Message> = self
            .state
            .messages
            .iter()
            .filter(|message| message.thread_id == thread.id)
            .collect();
        messages.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.as_str().cmp(b.id.as_str()))
        });
        EvidenceThread {
            thread_id: thread.id.as_str().to_string(),
            parent_type: thread.parent.node_type,
            parent_id: thread.parent.node_id.as_str().to_string(),
            status: thread.status.clone(),
            messages: messages
                .into_iter()
                .map(|message| FieldNote {
                    message_id: message.id.as_str().to_string(),
                    role: message.role.clone(),
                    created_at: message.created_at,
                    body: message.body.clone(),
                    refs: self.resolver.annotate(&message.body),
                })
                .collect(),
        }
    }

    pub(super) fn threads_for(
        &self,
        node_type: NodeType,
        node_id: &StableId,
    ) -> Vec<EvidenceThread> {
        self.state
            .threads
            .iter()
            .filter(|thread| {
                thread.parent.node_type == node_type && thread.parent.node_id == *node_id
            })
            .map(|thread| self.evidence_thread(thread))
            .collect()
    }
}
