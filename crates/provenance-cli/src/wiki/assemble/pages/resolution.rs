use crate::wiki::model::{PageId, PageLink, RecordKind, ResolutionPage, RuleCard};
use provenance_core::{NodeType, Resolution};

use super::super::context::Assembler;
use super::super::page_links::{requirement_link, resolution_link};

impl<'a> Assembler<'a> {
    pub(in crate::wiki::assemble) fn resolution_page(
        &self,
        resolution: &'a Resolution,
    ) -> ResolutionPage {
        let resolves: Vec<PageLink> = self
            .state
            .requirements
            .iter()
            .filter(|requirement| resolution.requirement_ids.contains(&requirement.id))
            .map(requirement_link)
            .collect();
        let spawned: Vec<PageLink> = self
            .state
            .requirements
            .iter()
            .filter(|requirement| requirement.spawned_by.as_ref() == Some(&resolution.id))
            .map(requirement_link)
            .collect();
        let produced_rules: Vec<RuleCard> = self
            .produced_rules_for_resolution(&resolution.id)
            .into_iter()
            .map(|rule| self.rule_card(rule))
            .collect();
        let superseded_by = self
            .state
            .resolutions
            .iter()
            .filter(|candidate| candidate.supersedes.contains(&resolution.id))
            .min_by_key(|candidate| candidate.id.as_str())
            .map(resolution_link);
        ResolutionPage {
            id: PageId::new(RecordKind::Resolution, resolution.id.as_str()),
            title: resolution.title.clone(),
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
            review_on: resolution.review_on.clone(),
            superseded_by,
            resolves,
            spawned,
            produced_rules,
            gaps: self.gaps_for(NodeType::Resolution, &resolution.id),
            threads: self.threads_for(NodeType::Resolution, &resolution.id),
        }
    }
}
