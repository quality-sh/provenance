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
                .ensure_cited_resolutions(
                    &question.scope_id,
                    &self.list_resolutions(&question.scope_id)?,
                    std::slice::from_ref(question),
                    &[],
                ),
            ReviewRecord::Rule(rule) if rule.status == RuleStatus::Active => self
                .ensure_cited_resolutions(
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
                self.ensure_cited_resolutions(
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
    /// approved, unless a reviewer accepted the current revision of that Resolution. Only the
    /// citations that the check reads need the review rows of their Resolution, and the check
    /// reads the review rows of the scope at most one time.
    fn ensure_cited_resolutions(
        &self,
        scope: &ScopeId,
        resolutions: &[Resolution],
        questions: &[Question],
        rules: &[Rule],
    ) -> anyhow::Result<()> {
        crate::review::ResolutionAcceptance::for_one_check(self, scope, |acceptance| {
            ensure_resolution_decisions(
                resolutions,
                |resolution| acceptance.accepts(resolution),
                questions,
                rules,
            )
        })
    }
}

/// Refuses an answered Question or an active Rule that cites a Resolution that is neither
/// approved nor `accepted`: a reviewer accepted its current revision. The check asks
/// `accepted` only for a cited Resolution that is not approved, in the order of the
/// citations.
pub(super) fn ensure_resolution_decisions(
    resolutions: &[Resolution],
    mut accepted: impl FnMut(&Resolution) -> anyhow::Result<bool>,
    questions: &[Question],
    rules: &[Rule],
) -> anyhow::Result<()> {
    for question in questions
        .iter()
        .filter(|q| q.status == QuestionStatus::Answered)
    {
        if let Some(id) = &question.resolution_id {
            ensure_approved(resolutions, &mut accepted, id, "question", &question.id)?;
        }
    }
    for rule in rules.iter().filter(|r| r.status == RuleStatus::Active) {
        for id in &rule.resolution_ids {
            ensure_approved(resolutions, &mut accepted, id, "rule", &rule.id)?;
        }
    }
    Ok(())
}

/// Refuses a citation of a Resolution that is not approved or does not exist, with a
/// message that names the citing record, the Resolution and its status, and what to do.
#[provenance_macros::rule("rule_unapproved_resolution_refusal_says_what_to_do")]
fn ensure_approved(
    resolutions: &[Resolution],
    accepted: &mut impl FnMut(&Resolution) -> anyhow::Result<bool>,
    id: &StableId,
    kind: &str,
    owner: &StableId,
) -> anyhow::Result<()> {
    let resolution = resolutions.iter().find(|resolution| &resolution.id == id);
    if let Some(resolution) = resolution {
        if resolution.status == ResolutionStatus::Approved || accepted(resolution)? {
            return Ok(());
        }
    }
    let state = resolution.map_or_else(
        || "does not exist".to_owned(),
        |resolution| format!("is {}", status_word(&resolution.status)),
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
    Err(crate::refusal::refuse!(
        (crate::write_error::WriteFailure::ResolutionNotApproved {
            resolution_id: id.to_owned(),
            message: message.clone(),
        }),
        "{message}"
    ))
}

/// The wire word of a Resolution status, read from its serde name.
fn status_word(status: &ResolutionStatus) -> String {
    match serde_json::to_value(status) {
        Ok(serde_json::Value::String(word)) => word,
        _ => format!("{status:?}"),
    }
}
