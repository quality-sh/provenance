use crate::wiki::model::{DecisionSection, PageId, RecordKind, RequirementPage, RuleCard};
use provenance_core::{NodeType, Requirement};

use super::super::context::Assembler;
use super::super::page_links::{reader_title, requirement_link};

impl<'a> Assembler<'a> {
    pub(in crate::wiki::assemble) fn requirement_page(
        &self,
        requirement: &'a Requirement,
    ) -> RequirementPage {
        let resolving = self.resolving_resolutions(&requirement.id);
        let decisions: Vec<DecisionSection> = resolving
            .iter()
            .map(|resolution| self.decision_section(resolution))
            .collect();
        let produced_rules: Vec<RuleCard> = self
            .produced_rules_for_requirement(&requirement.id)
            .into_iter()
            .map(|rule| self.rule_card(rule))
            .collect();
        let sources = self.requirement_sources(requirement);
        let gaps = self.gaps_for(NodeType::Requirement, &requirement.id);
        let mut threads = self.threads_for(NodeType::Requirement, &requirement.id);
        for resolution in &resolving {
            threads.extend(self.threads_for(NodeType::Resolution, &resolution.id));
        }
        RequirementPage {
            id: PageId::new(RecordKind::Requirement, requirement.id.as_str()),
            title: reader_title(&requirement.statement),
            status: requirement.status.clone(),
            statement: requirement.statement.clone(),
            description: requirement.description.clone(),
            fog: requirement.fog.clone(),
            domain_id: requirement
                .domain_id
                .as_ref()
                .map(|id| id.as_str().to_string()),
            domain_has_anchor: requirement.domain_id.is_some() && !self.state.domains.is_empty(),
            back_link: self.parent_of(&requirement.id).map(requirement_link),
            lineage: self.lineage(requirement),
            decisions,
            produced_rules,
            children: self
                .children_of(&requirement.id)
                .into_iter()
                .map(requirement_link)
                .collect(),
            siblings: self.sibling_requirements(&requirement.id),
            sources,
            gaps,
            threads,
        }
    }
}
