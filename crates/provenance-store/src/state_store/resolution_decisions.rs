//! A proposed decision cannot answer a Question or produce an active Rule.

use super::StateStore;
use provenance_core::{
    review::ReviewRecord, Question, QuestionStatus, Resolution, ResolutionStatus, Rule, RuleStatus,
    StableId,
};

impl StateStore {
    pub(super) fn validate_resolution_decision_write(
        &self,
        record: &ReviewRecord,
    ) -> anyhow::Result<()> {
        match record {
            ReviewRecord::Question(question) if question.status == QuestionStatus::Answered => {
                ensure_resolution_decisions(
                    &self.list_resolutions(&question.scope_id)?,
                    std::slice::from_ref(question),
                    &[],
                )
            }
            ReviewRecord::Rule(rule) if rule.status == RuleStatus::Active => {
                ensure_resolution_decisions(
                    &self.list_resolutions(&rule.scope_id)?,
                    &[],
                    std::slice::from_ref(rule),
                )
            }
            _ => Ok(()),
        }
    }
}

pub(super) fn ensure_resolution_decisions(
    resolutions: &[Resolution],
    questions: &[Question],
    rules: &[Rule],
) -> anyhow::Result<()> {
    for question in questions
        .iter()
        .filter(|q| q.status == QuestionStatus::Answered)
    {
        if let Some(id) = &question.resolution_id {
            ensure_approved(resolutions, id, "question", &question.id)?;
        }
    }
    for rule in rules.iter().filter(|r| r.status == RuleStatus::Active) {
        for id in &rule.resolution_ids {
            ensure_approved(resolutions, id, "rule", &rule.id)?;
        }
    }
    Ok(())
}

fn ensure_approved(
    resolutions: &[Resolution],
    id: &StableId,
    kind: &str,
    owner: &StableId,
) -> anyhow::Result<()> {
    crate::write_error::ensure!(
        InvalidUpdate,
        resolutions.iter().any(|resolution| {
            &resolution.id == id && resolution.status == ResolutionStatus::Approved
        }),
        "{kind} {} is refused: resolution {} must be approved",
        owner.as_str(),
        id.as_str()
    );
    Ok(())
}
