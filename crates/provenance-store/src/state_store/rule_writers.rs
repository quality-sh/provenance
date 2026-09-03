use super::{CreateResolutionInput, CreateRuleInput, StateStore};
use crate::shards;
use provenance_core::{EdgeType, NodeType, Resolution, Rule, SUPPORTED_SCHEMA_VERSION};

impl StateStore {
    pub fn create_resolution(&self, input: CreateResolutionInput) -> anyhow::Result<Resolution> {
        self.with_repository_publication(|| self.write_resolution(input))
    }

    fn write_resolution(&self, input: CreateResolutionInput) -> anyhow::Result<Resolution> {
        let CreateResolutionInput {
            scope_id,
            id,
            title,
            requirement_id,
            position,
            rationale,
            status,
            context,
            enforcement,
            confidence,
            inputs,
            made_by,
            approved_by,
            approved_at,
            superseded_by,
            origin_thread,
            origin_message,
        } = input;
        if let Some(requirement_id) = &requirement_id {
            anyhow::ensure!(
                self.list_requirements(&scope_id)?
                    .iter()
                    .any(|requirement| &requirement.id == requirement_id),
                "requirement does not exist"
            );
        }
        let path = shards::resolutions_path(&self.layout, &scope_id);
        let resolution = self.mutate_jsonl_records(&path, |records: &mut Vec<Resolution>| {
            let resolution = Resolution {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id: id.clone(),
                title,
                position,
                rationale,
                status,
                context,
                enforcement,
                confidence,
                inputs,
                made_by,
                approved_by,
                approved_at,
                superseded_by,
                review_on: None,
                requirement_ids: Vec::new(),
                supersedes: Vec::new(),
                origin_thread,
                origin_message,
            };
            anyhow::ensure!(
                !records.iter().any(|record| record.id == resolution.id),
                "resolution already exists"
            );
            records.push(resolution.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(resolution)
        })?;
        if let Some(requirement_id) = requirement_id {
            self.add_edge(
                scope_id.clone(),
                EdgeType::Needs,
                NodeType::Requirement,
                requirement_id.clone(),
                NodeType::Resolution,
                id.clone(),
            )?;
            self.add_edge(
                scope_id,
                EdgeType::Resolves,
                NodeType::Resolution,
                id,
                NodeType::Requirement,
                requirement_id,
            )?;
        }
        Ok(resolution)
    }

    pub fn create_rule(&self, input: CreateRuleInput) -> anyhow::Result<Rule> {
        self.with_repository_publication(|| self.write_rule(input))
    }

    fn write_rule(&self, input: CreateRuleInput) -> anyhow::Result<Rule> {
        let CreateRuleInput {
            scope_id,
            id,
            name,
            description,
            requirement_id,
            resolution_id,
            statement,
            status,
            severity,
            source_document,
            source_section,
            origin_thread,
            origin_message,
        } = input;
        super::statement_policy::ensure_statement_is_writable(&self.layout, &statement)?;
        if let Some(requirement_id) = &requirement_id {
            anyhow::ensure!(
                self.list_requirements(&scope_id)?
                    .iter()
                    .any(|requirement| &requirement.id == requirement_id),
                "requirement does not exist"
            );
        }
        if let Some(resolution_id) = &resolution_id {
            anyhow::ensure!(
                self.list_resolutions(&scope_id)?
                    .iter()
                    .any(|resolution| &resolution.id == resolution_id),
                "resolution does not exist"
            );
        }
        let path = shards::rules_path(&self.layout, &scope_id);
        let rule = self.mutate_jsonl_records(&path, |records: &mut Vec<Rule>| {
            let rule = Rule {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id: id.clone(),
                declared_by: None,
                declaration_address: None,
                retired: false,
                name,
                description,
                statement,
                status,
                severity,
                source_document,
                source_section,
                requirement_ids: Vec::new(),
                resolution_ids: Vec::new(),
                origin_thread,
                origin_message,
            };
            anyhow::ensure!(
                !records.iter().any(|record| record.id == rule.id),
                "rule already exists"
            );
            records.push(rule.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(rule)
        })?;
        if let Some(requirement_id) = requirement_id {
            self.add_edge(
                scope_id.clone(),
                EdgeType::Produces,
                NodeType::Requirement,
                requirement_id,
                NodeType::Rule,
                id.clone(),
            )?;
        }
        if let Some(resolution_id) = resolution_id {
            self.add_edge(
                scope_id,
                EdgeType::Produces,
                NodeType::Resolution,
                resolution_id,
                NodeType::Rule,
                id,
            )?;
        }
        Ok(rule)
    }
}
