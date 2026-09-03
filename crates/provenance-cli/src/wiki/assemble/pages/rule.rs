use crate::wiki::model::{PageId, PageLink, RecordKind, RulePage};
use provenance_core::{NodeType, Requirement, Resolution, Rule};

use super::super::context::Assembler;
use super::super::page_links::{requirement_link, resolution_link, rule_title, source_link};

impl<'a> Assembler<'a> {
    pub(in crate::wiki::assemble) fn rule_page(&self, rule: &'a Rule) -> RulePage {
        let producing_resolutions: Vec<&Resolution> = self
            .state
            .resolutions
            .iter()
            .filter(|resolution| rule.resolution_ids.contains(&resolution.id))
            .collect();
        let producing_requirements: Vec<&Requirement> = self
            .state
            .requirements
            .iter()
            .filter(|requirement| rule.requirement_ids.contains(&requirement.id))
            .collect();
        let produced_by: Vec<PageLink> = producing_resolutions
            .iter()
            .copied()
            .map(resolution_link)
            .chain(producing_requirements.iter().copied().map(requirement_link))
            .collect();
        // Which requirements a rule answers to is decided once, by the
        // forward traversal; this page reads that decision inverted rather
        // than walking the lists backwards itself.
        let upstream_requirements: &[&Requirement] = self.requirements_behind_rule(&rule.id);
        let sources: Vec<PageLink> = self
            .state
            .sources
            .iter()
            .filter(|source| {
                upstream_requirements.iter().any(|requirement| {
                    requirement
                        .source_refs
                        .iter()
                        .any(|reference| reference.source_id == source.id)
                })
            })
            .map(source_link)
            .collect();
        RulePage {
            id: PageId::new(RecordKind::Rule, rule.id.as_str()),
            title: rule_title(rule),
            statement: rule.statement.clone(),
            description: rule.description.clone(),
            status: rule.status.clone(),
            severity: rule.severity.clone(),
            code_scan: self.code_scan(),
            implementations: self.implementations(rule.id.as_str()),
            verifications: self.verification_sites(rule.id.as_str()),
            produced_by,
            requirements: upstream_requirements
                .iter()
                .copied()
                .map(requirement_link)
                .collect(),
            sources,
            gaps: self.gaps_for(NodeType::Rule, &rule.id),
            threads: self.threads_for(NodeType::Rule, &rule.id),
        }
    }
}
