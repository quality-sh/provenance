use super::writers::sorted_ids;
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
            requirement_ids,
            supersedes,
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
        anyhow::ensure!(
            !requirement_ids.is_empty(),
            "a resolution needs one requirement"
        );
        for requirement_id in &requirement_ids {
            self.ensure_node_exists(&scope_id, NodeType::Requirement, requirement_id)?;
        }
        for older in &supersedes {
            self.ensure_node_exists(&scope_id, NodeType::Resolution, older)?;
        }
        let requirement_ids = sorted_ids(requirement_ids);
        let supersedes = sorted_ids(supersedes);
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
                requirement_ids: requirement_ids.clone(),
                supersedes,
                superseded_by,
                review_on: None,
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
        for requirement_id in requirement_ids {
            self.add_edge(
                scope_id.clone(),
                EdgeType::Needs,
                NodeType::Requirement,
                requirement_id.clone(),
                NodeType::Resolution,
                id.clone(),
            )?;
            self.add_edge(
                scope_id.clone(),
                EdgeType::Resolves,
                NodeType::Resolution,
                id.clone(),
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
            requirement_ids,
            resolution_ids,
            statement,
            status,
            severity,
            source_document,
            source_section,
            origin_thread,
            origin_message,
        } = input;
        super::statement_policy::ensure_statement_is_writable(&self.layout, &statement)?;
        anyhow::ensure!(!requirement_ids.is_empty(), "a rule needs one requirement");
        for requirement_id in &requirement_ids {
            self.ensure_node_exists(&scope_id, NodeType::Requirement, requirement_id)?;
        }
        for resolution_id in &resolution_ids {
            self.ensure_node_exists(&scope_id, NodeType::Resolution, resolution_id)?;
        }
        let requirement_ids = sorted_ids(requirement_ids);
        let resolution_ids = sorted_ids(resolution_ids);
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
                requirement_ids: requirement_ids.clone(),
                resolution_ids: resolution_ids.clone(),
                source_document,
                source_section,
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
        for requirement_id in requirement_ids {
            self.add_edge(
                scope_id.clone(),
                EdgeType::Produces,
                NodeType::Requirement,
                requirement_id,
                NodeType::Rule,
                id.clone(),
            )?;
        }
        for resolution_id in resolution_ids {
            self.add_edge(
                scope_id.clone(),
                EdgeType::Produces,
                NodeType::Resolution,
                resolution_id,
                NodeType::Rule,
                id.clone(),
            )?;
        }
        Ok(rule)
    }
}
