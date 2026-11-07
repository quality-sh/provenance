//! An answered Question or an active Rule cites only an approved Resolution, or a draft,
//! review or proposed Resolution whose current revision a reviewer accepted.

use super::StateStore;
use provenance_core::{
    review::ReviewRecord, Question, QuestionStatus, Resolution, ResolutionStatus, Rule, RuleStatus,
    ScopeId, StableId,
};

impl StateStore {
    pub(super) fn validate_resolution_decision_write(
        &self,
        record: &ReviewRecord,
    ) -> anyhow::Result<()> {
        match record {
            ReviewRecord::Question(question) if question.status == QuestionStatus::Answered => self
                .ensure_resolution_decisions(
                    &question.scope_id,
                    &self.list_resolutions(&question.scope_id)?,
                    std::slice::from_ref(question),
                    &[],
                ),
            ReviewRecord::Rule(rule) if rule.status == RuleStatus::Active => self
                .ensure_resolution_decisions(
                    &rule.scope_id,
                    &self.list_resolutions(&rule.scope_id)?,
                    &[],
                    std::slice::from_ref(rule),
                ),
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
                self.ensure_resolution_decisions(
                    &resolution.scope_id,
                    &resolutions,
                    &questions,
                    &rules,
                )
            }
            _ => Ok(()),
        }
    }

    /// Refuses an answered Question or an active Rule that cites a Resolution that is not
    /// approved, unless a reviewer accepted the current revision of that Resolution.
    pub(super) fn ensure_resolution_decisions(
        &self,
        scope: &ScopeId,
        resolutions: &[Resolution],
        questions: &[Question],
        rules: &[Rule],
    ) -> anyhow::Result<()> {
        let answered = questions
            .iter()
            .filter(|q| q.status == QuestionStatus::Answered)
            .filter_map(|q| q.resolution_id.as_ref().map(|id| (id, "question", &q.id)));
        let active = rules
            .iter()
            .filter(|r| r.status == RuleStatus::Active)
            .flat_map(|r| r.resolution_ids.iter().map(move |id| (id, "rule", &r.id)));
        let citations = answered.chain(active).collect::<Vec<_>>();
        let cited = resolutions
            .iter()
            .filter(|resolution| resolution.status != ResolutionStatus::Approved)
            .filter(|resolution| citations.iter().any(|(id, _, _)| **id == resolution.id))
            .cloned()
            .collect::<Vec<_>>();
        let accepted = self.accepted_resolution_ids(scope, &cited)?;
        for (id, kind, owner) in citations {
            ensure_approved(resolutions, &accepted, id, kind, owner)?;
        }
        Ok(())
    }
}

/// Refuses a citation of a Resolution that is not approved or does not exist, with a
/// message that names the citing record, the Resolution and its status, and what to do.
#[provenance_macros::rule("rule_unapproved_resolution_refusal_says_what_to_do")]
fn ensure_approved(
    resolutions: &[Resolution],
    accepted: &[StableId],
    id: &StableId,
    kind: &str,
    owner: &StableId,
) -> anyhow::Result<()> {
    let status = resolutions
        .iter()
        .find(|resolution| &resolution.id == id)
        .map(|resolution| resolution.status.clone());
    if status == Some(ResolutionStatus::Approved) || accepted.contains(id) {
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
    // The text error carries the same message, so a command without JSON output, such as a
    // scope import, also says what to do.
    let text = anyhow::anyhow!("{message}");
    Err(crate::write_error::SourceFailure::wrap(
        crate::write_error::WriteFailure::ResolutionNotApproved {
            resolution_id: id.to_owned(),
            message,
        },
        text,
    ))
}

/// The wire word of a Resolution status, read from its serde name.
fn status_word(status: &ResolutionStatus) -> String {
    match serde_json::to_value(status) {
        Ok(serde_json::Value::String(word)) => word,
        _ => format!("{status:?}"),
    }
}
