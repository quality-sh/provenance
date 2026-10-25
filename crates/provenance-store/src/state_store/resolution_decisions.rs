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
            ReviewRecord::Resolution(resolution)
                if resolution.status != ResolutionStatus::Approved =>
            {
                let mut resolutions = self.list_resolutions(&resolution.scope_id)?;
                resolutions.retain(|stored| stored.id != resolution.id);
                resolutions.push(resolution.clone());
                let questions = self
                    .list_questions(&resolution.scope_id)?
                    .into_iter()
                    .filter(|question| question.resolution_id.as_ref() == Some(&resolution.id))
                    .collect::<Vec<_>>();
                let rules = self
                    .list_rules(&resolution.scope_id)?
                    .into_iter()
                    .filter(|rule| rule.resolution_ids.contains(&resolution.id))
                    .collect::<Vec<_>>();
                ensure_resolution_decisions(&resolutions, &questions, &rules)
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

/// Refuses a citation of a Resolution that is not approved or does not exist, with a
/// message that names the citing record, the Resolution and its status, and what to do.
#[provenance_macros::rule("rule_unapproved_resolution_refusal_says_what_to_do")]
fn ensure_approved(
    resolutions: &[Resolution],
    id: &StableId,
    kind: &str,
    owner: &StableId,
) -> anyhow::Result<()> {
    let status = resolutions
        .iter()
        .find(|resolution| &resolution.id == id)
        .map(|resolution| resolution.status.clone());
    if status == Some(ResolutionStatus::Approved) {
        return Ok(());
    }
    let state = status.map_or_else(
        || "does not exist".to_owned(),
        |status| format!("is {}", status_word(&status)),
    );
    let id = id.as_str();
    let message = format!(
        "{kind} {} cites resolution {id}, which {state}. An answered Question or an active \
         Rule can cite only an approved Resolution. Until a reviewer approves {id}, keep the \
         Rule in draft or review, or leave the Question open, or remove the citation and name \
         {id} in the text.",
        owner.as_str()
    );
    Err(crate::write_error::SourceFailure::wrap(
        crate::write_error::WriteFailure::ResolutionNotApproved {
            resolution_id: id.to_owned(),
            message,
        },
        anyhow::anyhow!(
            "{kind} {} is refused: resolution {id} must be approved",
            owner.as_str()
        ),
    ))
}

/// The wire word of a Resolution status.
const fn status_word(status: &ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Draft => "draft",
        ResolutionStatus::Review => "review",
        ResolutionStatus::Proposed => "proposed",
        ResolutionStatus::Approved => "approved",
        ResolutionStatus::Rejected => "rejected",
        ResolutionStatus::Revised => "revised",
        ResolutionStatus::Superseded => "superseded",
        ResolutionStatus::Abandoned => "abandoned",
    }
}
