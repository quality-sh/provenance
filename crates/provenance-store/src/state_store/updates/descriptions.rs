use super::{
    inputs::{
        BoundaryClearField, DomainClearField, RequirementClearField, UpdateBoundaryInput,
        UpdateDomainInput, UpdateRequirementInput,
    },
    invalid, missing, optional, owner_matches, required_text, set,
};
use crate::{
    shards,
    state_store::{RequirementReviewInput, StateStore},
};
use provenance_core::{Boundary, Domain, NodeType, Requirement};

impl StateStore {
    pub fn update_requirement(&self, input: UpdateRequirementInput) -> anyhow::Result<Requirement> {
        self.with_repository_publication(|| {
            if let Some(id) = &input.domain_id {
                self.ensure_node_exists(&input.scope_id, NodeType::Domain, id, "domain_id")?;
            }
            if let Some(statement) = &input.statement {
                required_text(statement)?;
                super::super::statement_policy::ensure_statement_is_writable(
                    &self.layout,
                    statement,
                )?;
            }
            if let Some(fog) = &input.fog {
                required_text(fog)?;
            }
            let rule_ids = self.rule_ids_for_requirement(&input.scope_id, &input.id)?;
            self.list_requirement_reviews(&input.scope_id)?;
            let changed_at = super::super::requirement_reviews::now_millis()?;
            let path = shards::requirements_path(&self.layout, &input.scope_id);
            let mut before = String::new();
            let record = self.mutate_graph_record(&path, |records: &mut Vec<Requirement>| {
                let record = records
                    .iter_mut()
                    .find(|r| r.id == input.id)
                    .ok_or_else(missing)?;
                owner_matches(record.declared_by.as_deref(), input.declared_by.as_deref())?;
                before.clone_from(&record.statement);
                set(&mut record.statement, input.statement);
                set(&mut record.status, input.status);
                optional(
                    &mut record.description,
                    input.description,
                    input
                        .clear_fields
                        .contains(&RequirementClearField::Description),
                )?;
                optional(
                    &mut record.fog,
                    input.fog,
                    input.clear_fields.contains(&RequirementClearField::Fog),
                )?;
                optional(
                    &mut record.domain_id,
                    input.domain_id,
                    input
                        .clear_fields
                        .contains(&RequirementClearField::DomainId),
                )?;
                Ok(record.clone())
            })?;
            if before != record.statement {
                let reviews = rule_ids
                    .into_iter()
                    .map(|rule_id| RequirementReviewInput {
                        rule_id,
                        requirement_id: record.id.clone(),
                        field: "statement".into(),
                        before: before.clone(),
                        after: record.statement.clone(),
                        changed_at,
                    })
                    .collect();
                self.record_requirement_reviews(&input.scope_id, reviews)
                    .map_err(crate::write_error::publication_started)?;
            }
            Ok(record)
        })
    }

    pub fn update_domain(&self, input: UpdateDomainInput) -> anyhow::Result<Domain> {
        let path = shards::domains_path(&self.layout, &input.scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Domain>| {
            if let Some(name) = &input.name {
                required_text(name)?;
                if records.iter().any(|r| r.id != input.id && r.name == *name) {
                    return Err(invalid("domain name already exists"));
                }
            }
            let record = records
                .iter_mut()
                .find(|r| r.id == input.id)
                .ok_or_else(missing)?;
            set(&mut record.name, input.name);
            optional(
                &mut record.description,
                input.description,
                input.clear_fields.contains(&DomainClearField::Description),
            )?;
            optional(
                &mut record.color,
                input.color,
                input.clear_fields.contains(&DomainClearField::Color),
            )?;
            Ok(record.clone())
        })
    }

    pub fn update_boundary(&self, input: UpdateBoundaryInput) -> anyhow::Result<Boundary> {
        self.with_repository_publication(|| {
            if let Some(reference) = &input.source_ref {
                self.ensure_node_exists(
                    &input.scope_id,
                    NodeType::Source,
                    &reference.source_id,
                    "source_ref",
                )?;
            }
            if let Some(statement) = &input.statement {
                required_text(statement)?;
            }
            let path = shards::boundaries_path(&self.layout, &input.scope_id);
            self.mutate_jsonl_records(&path, |records: &mut Vec<Boundary>| {
                let record = records
                    .iter_mut()
                    .find(|r| r.id == input.id)
                    .ok_or_else(missing)?;
                set(&mut record.statement, input.statement);
                optional(
                    &mut record.source_ref,
                    input.source_ref,
                    input.clear_fields.contains(&BoundaryClearField::SourceRef),
                )?;
                Ok(record.clone())
            })
        })
    }
}
