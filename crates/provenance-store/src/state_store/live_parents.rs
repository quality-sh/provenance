//! A new parent link cannot point at an archived Requirement.

use super::StateStore;
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{review::ReviewRecord, RequirementStatus, StableId};

impl StateStore {
    /// Refuses each Requirement link that the write adds to an archived Requirement.
    #[provenance_macros::rule("rule_live_dependents_have_live_parents")]
    pub(super) fn validate_new_requirement_links(
        &self,
        record: &ReviewRecord,
        previous: Option<&ReviewRecord>,
    ) -> anyhow::Result<()> {
        let kept = previous.map(requirement_parents).unwrap_or_default();
        let added = requirement_parents(record)
            .into_iter()
            .filter(|id| !kept.contains(id))
            .collect::<Vec<_>>();
        if added.is_empty() {
            return Ok(());
        }
        let archived = self
            .list_requirements(record.scope_id())?
            .into_iter()
            .find(|parent| {
                parent.status == RequirementStatus::Archived && added.contains(&parent.id)
            });
        archived.map_or(Ok(()), |parent| {
            Err(SourceFailure::wrap(
                WriteFailure::InvalidUpdate,
                anyhow::anyhow!(
                    "Requirement {} is archived; a new link cannot point to it",
                    parent.id.as_str()
                ),
            ))
        })
    }
}

fn requirement_parents(record: &ReviewRecord) -> Vec<StableId> {
    match record {
        ReviewRecord::Requirement(record) => record.refines.clone(),
        ReviewRecord::Rule(record) => record.requirement_ids.clone(),
        ReviewRecord::Resolution(record) => record.requirement_ids.clone(),
        ReviewRecord::Topic(record) => vec![record.requirement_id.clone()],
        ReviewRecord::Question(record) => vec![record.requirement_id.clone()],
        ReviewRecord::Boundary(record) => vec![record.requirement_id.clone()],
        _ => Vec::new(),
    }
}
