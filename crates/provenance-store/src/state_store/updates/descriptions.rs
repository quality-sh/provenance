use super::{
    inputs::{RequirementClearField, UpdateRequirementInput},
    invalid, missing, optional, owner_matches, required_text, set,
};
use crate::{
    shards,
    state_store::{RequirementReviewInput, StateStore},
};
use provenance_core::{Requirement, RequirementStatus};
use provenance_macros::rule;

impl StateStore {
    /// Writes one review per produced Rule when the Requirement statement changes.
    #[rule("rule_review_record_per_rule")]
    pub(crate) fn apply_requirement_update(
        &self,
        input: UpdateRequirementInput,
    ) -> anyhow::Result<Requirement> {
        self.with_repository_publication(|held| {
            if let Some(statement) = &input.statement {
                required_text(statement)?;
                super::super::statement_policy::ensure_statement_is_writable(
                    &held.layout,
                    statement,
                )?;
            }
            let rule_ids = held.rule_ids_for_requirement(&input.scope_id, &input.id)?;
            held.list_requirement_reviews(&input.scope_id)?;
            let changed_at = super::super::requirement_reviews::now_millis()?;
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
                held.record_requirement_reviews(&input.scope_id, reviews)
                    .map_err(crate::write_error::publication_started)?;
            }
            Ok(record)
        })
    }
}
