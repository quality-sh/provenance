use super::{
    inputs::{RequirementClearField, UpdateRequirementInput},
    invalid, missing, optional, owner_matches, required_text, set,
};
use crate::{
    shards,
    state_store::{requirement_reviews::Restatement, StateStore},
};
use provenance_core::{Requirement, RequirementStatus};

impl StateStore {
    /// Writes one review per produced Rule when the Requirement statement changes.
    pub(crate) fn apply_requirement_update(
        &self,
        input: UpdateRequirementInput,
    ) -> anyhow::Result<Requirement> {
        self.with_repository_publication(|held| {
            if let Some(statement) = &input.statement {
                required_text(statement)?;
                super::super::statement_policy::ensure_statement_is_writable(held, statement)?;
            }
            let rule_ids = held.rule_ids_for_requirement(&input.scope_id, &input.id)?;
            held.list_requirement_reviews(&input.scope_id)?;
            let path = shards::requirements_path(&held.layout, &input.scope_id);
            let mut before = String::new();
            let mut was_archived = false;
            let record = held.mutate_graph_record(&path, |records: &mut Vec<Requirement>| {
                let record = records
                    .iter_mut()
                    .find(|r| r.id == input.id)
                    .ok_or_else(missing)?;
                owner_matches(record.declared_by.as_deref(), input.declared_by.as_deref())?;
                before.clone_from(&record.statement);
                was_archived = record.status == RequirementStatus::Archived;
                set(&mut record.statement, input.statement);
                set(&mut record.status, input.status);
                set(
                    &mut record.archived_in_commit,
                    input.archived_in_commit.map(Some),
                );
                optional(
                    &mut record.description,
                    input.description,
                    input
                        .clear_fields
                        .contains(&RequirementClearField::Description),
                )?;
                Ok(record.clone())
            })?;
            if record.status == RequirementStatus::Archived && !was_archived {
                let stamp = record.archived_in_commit.as_ref().ok_or_else(|| {
                    invalid("archived_in_commit is required for an archived Requirement")
                })?;
                held.archive_dependents(&record.scope_id, &record.id, stamp)?;
            }
            if before != record.statement {
                let restatement = Restatement {
                    requirement_id: record.id.clone(),
                    before,
                    after: record.statement.clone(),
                    rule_ids,
                };
                held.record_restatements(&input.scope_id, vec![restatement])
                    .map_err(crate::write_error::publication_started)?;
            }
            Ok(record)
        })
    }
}
