use super::{
    inputs::{RuleClearField, SourceClearField, UpdateRuleInput, UpdateSourceInput},
    missing, optional, owner_matches, required_text, set,
};
use crate::{shards, state_store::StateStore};
use provenance_core::{validate_optional_commit_pin, Rule, Source};

impl StateStore {
    pub fn update_source(&self, input: UpdateSourceInput) -> anyhow::Result<Source> {
        let path = shards::sources_path(&self.layout, &input.scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Source>| {
            let record = records
                .iter_mut()
                .find(|r| r.id == input.id)
                .ok_or_else(missing)?;
            owner_matches(record.declared_by.as_deref(), input.declared_by.as_deref())?;
            if let Some(name) = &input.name {
                required_text(name)?;
            }
            set(&mut record.name, input.name);
            set(&mut record.source_type, input.source_type);
            set(&mut record.retired, input.retired);
            optional(
                &mut record.url,
                input.url,
                input.clear_fields.contains(&SourceClearField::Url),
            )?;
            optional(
                &mut record.reference,
                input.reference,
                input.clear_fields.contains(&SourceClearField::Reference),
            )?;
            optional(
                &mut record.commit_pin,
                input.commit_pin,
                input.clear_fields.contains(&SourceClearField::CommitPin),
            )?;
            optional(
                &mut record.effective_date,
                input.effective_date,
                input
                    .clear_fields
                    .contains(&SourceClearField::EffectiveDate),
            )?;
            optional(
                &mut record.review_date,
                input.review_date,
                input.clear_fields.contains(&SourceClearField::ReviewDate),
            )?;
            record.commit_pin =
                validate_optional_commit_pin(record.commit_pin.take()).map_err(|error| {
                    crate::write_error::SourceFailure::wrap(
                        crate::write_error::WriteFailure::InvalidCommitPin,
                        error,
                    )
                })?;
            Ok(record.clone())
        })
    }

    pub fn update_rule(&self, input: UpdateRuleInput) -> anyhow::Result<Rule> {
        self.with_repository_publication(|| {
            let path = shards::rules_path(&self.layout, &input.scope_id);
            self.mutate_jsonl_records(&path, |records: &mut Vec<Rule>| {
                let record = records
                    .iter_mut()
                    .find(|r| r.id == input.id)
                    .ok_or_else(missing)?;
                owner_matches(record.declared_by.as_deref(), input.declared_by.as_deref())?;
                if let Some(statement) = &input.statement {
                    required_text(statement)?;
                    super::super::statement_policy::ensure_statement_is_writable(
                        &self.layout,
                        statement,
                    )?;
                }
                set(&mut record.statement, input.statement);
                set(&mut record.status, input.status);
                set(&mut record.severity, input.severity);
                set(&mut record.retired, input.retired);
                optional(
                    &mut record.name,
                    input.name,
                    input.clear_fields.contains(&RuleClearField::Name),
                )?;
                optional(
                    &mut record.description,
                    input.description,
                    input.clear_fields.contains(&RuleClearField::Description),
                )?;
                optional(
                    &mut record.source_document,
                    input.source_document,
                    input.clear_fields.contains(&RuleClearField::SourceDocument),
                )?;
                optional(
                    &mut record.source_section,
                    input.source_section,
                    input.clear_fields.contains(&RuleClearField::SourceSection),
                )?;
                Ok(record.clone())
            })
        })
    }
}
