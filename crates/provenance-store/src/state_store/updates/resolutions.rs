use super::{
    inputs::{ResolutionClearField, UpdateResolutionInput},
    invalid, missing, optional, required_text, set, validate_final_relations,
};
use crate::{publication::with_staged_state, review, shards, state_store::StateStore};
use provenance_core::{
    validate_optional_confidence_score, validate_resolution_input_content, Resolution,
};

impl StateStore {
    pub fn update_resolution(&self, input: UpdateResolutionInput) -> anyhow::Result<Resolution> {
        with_staged_state(&self.layout, false, |layout| {
            Self::new(layout.clone()).prepare_resolution_update(input)
        })
    }

    fn prepare_resolution_update(
        &self,
        input: UpdateResolutionInput,
    ) -> anyhow::Result<Resolution> {
        let scope = input.scope_id.clone();
        let records = self.list_resolutions(&scope)?;
        records
            .iter()
            .find(|record| record.id == input.id)
            .ok_or_else(missing)?;
        review::relationships::validate_list_edit_targets(
            self,
            &scope,
            &records,
            &input.id,
            "requirement_ids",
            input.requirement_ids.as_ref(),
        )?;
        review::relationships::validate_list_edit_targets(
            self,
            &scope,
            &records,
            &input.id,
            "supersedes",
            input.supersedes.as_ref(),
        )?;
        let path = shards::resolutions_path(&self.layout, &input.scope_id);
        let record = self.mutate_graph_record(&path, |records: &mut Vec<Resolution>| {
            let record = records
                .iter_mut()
                .find(|r| r.id == input.id)
                .ok_or_else(missing)?;
            review::relationships::expand_list(
                &mut record.requirement_ids,
                input.requirement_ids.as_ref(),
            )?;
            review::relationships::expand_list(
                &mut record.supersedes,
                input.supersedes.as_ref(),
            )?;
            for text in [&input.title, &input.position, &input.rationale]
                .into_iter()
                .flatten()
            {
                required_text(text)?;
            }
            set(&mut record.title, input.title);
            set(&mut record.position, input.position);
            set(&mut record.rationale, input.rationale);
            set(&mut record.status, input.status);
            if let Some(inputs) = input.inputs {
                for value in &inputs {
                    validate_resolution_input_content(&value.reference, &value.summary)
                        .map_err(|error| invalid(&error.to_string()))?;
                }
                record.inputs = inputs;
            }
            optional(
                &mut record.context,
                input.context,
                input.clear_fields.contains(&ResolutionClearField::Context),
            )?;
            optional(
                &mut record.enforcement,
                input.enforcement,
                input
                    .clear_fields
                    .contains(&ResolutionClearField::Enforcement),
            )?;
            optional(
                &mut record.confidence,
                input.confidence,
                input
                    .clear_fields
                    .contains(&ResolutionClearField::Confidence),
            )?;
            optional(
                &mut record.made_by,
                input.made_by,
                input.clear_fields.contains(&ResolutionClearField::MadeBy),
            )?;
            optional(
                &mut record.approved_by,
                input.approved_by,
                input
                    .clear_fields
                    .contains(&ResolutionClearField::ApprovedBy),
            )?;
            optional(
                &mut record.approved_at,
                input.approved_at,
                input
                    .clear_fields
                    .contains(&ResolutionClearField::ApprovedAt),
            )?;
            optional(
                &mut record.review_on,
                input.review_on,
                input.clear_fields.contains(&ResolutionClearField::ReviewOn),
            )?;
            validate_optional_confidence_score(record.confidence)
                .map_err(|error| invalid(&error.to_string()))?;
            Ok(record.clone())
        })?;
        validate_final_relations(self, &scope, &record)?;
        Ok(record)
    }
}
