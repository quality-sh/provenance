//! Status requests: a change of a record status to active or archived waits
//! for a reviewer. The request opens a review submission that names the
//! requested status, and an approval of that submission sets the status.

use super::{
    automatic_submission::AUTHORING_ACTOR, decision_state::check_request_size, owner_matches,
    publication::with_record_state, SubmitRecordReview,
};
use crate::{
    shards,
    state_store::{CreateRuleInput, RecordStamp, StateStore, UpdateRuleInput},
    write_error::WriteFailure,
};
use provenance_core::{
    review::{RequestedStatus, ReviewRecord, StatusRequest},
    DispositionDecision, NodeType, RecordRevisionBinding, Rule, RuleStatus,
};
use provenance_macros::rule;

/// The longest summary a status request copies from the record.
const SUMMARY_CHARS: usize = 1000;

const fn requested_rule_status(status: RuleStatus) -> Option<RequestedStatus> {
    match status {
        RuleStatus::Active => Some(RequestedStatus::Active),
        RuleStatus::Archived => Some(RequestedStatus::Archived),
        RuleStatus::Draft | RuleStatus::Review | RuleStatus::Deprecated => None,
    }
}

const fn rule_status(status: RequestedStatus) -> RuleStatus {
    match status {
        RequestedStatus::Active => RuleStatus::Active,
        RequestedStatus::Archived => RuleStatus::Archived,
    }
}

/// Refuses an update that requests a status and also changes another field.
#[rule("rule_status_request_changes_one_field")]
fn ensure_status_only(other_fields: bool) -> anyhow::Result<()> {
    if other_fields {
        return Err(crate::refusal::refuse!(
            InvalidUpdate,
            "an update that sets the status to active or archived cannot change other fields: \
             send the other fields in a separate update"
        ));
    }
    Ok(())
}

/// Refuses a stale edit guard on a status request.
fn check_etag(record: &ReviewRecord, expected: Option<&str>) -> anyhow::Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let current_etag = super::save::etag(record)?;
    if expected != current_etag {
        return Err(crate::refusal::refuse!(
            (WriteFailure::RecordEditConflict {
                record_kind: record.kind(),
                current_etag,
            }),
            "stale record edit etag"
        ));
    }
    Ok(())
}

/// The status request that an approval applies. A rejection or a
/// withdrawal applies nothing, so the record keeps its status.
#[rule("rule_status_refusal_keeps_status")]
pub(super) fn accepted_request(
    decision: DispositionDecision,
    binding: &RecordRevisionBinding,
) -> Option<&StatusRequest> {
    (decision == DispositionDecision::Accepted)
        .then_some(binding.status_request.as_ref())
        .flatten()
}

/// Runs the direct write of a status request in a stage that it drops, so
/// the request gets the refusal that the same change without review gets
/// and opens no review submission.
#[rule("rule_status_request_refused_like_direct_write")]
fn dry_run<R>(
    held: &StateStore,
    write: impl FnOnce(&StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<()> {
    crate::publication::with_staged_state(held, true, RecordStamp::ResolveFromRepository, write)?;
    Ok(())
}

fn rule_fields_changed(input: &UpdateRuleInput) -> bool {
    input.name.is_some()
        || input.description.is_some()
        || input.statement.is_some()
        || input.severity.is_some()
        || input.source_document.is_some()
        || input.source_section.is_some()
        || input.requirement_ids.is_some()
        || input.resolution_ids.is_some()
        || !input.clear_fields.is_empty()
}

impl StateStore {
    /// Runs the `update-rule` operation. A change of the status to active or
    /// archived opens a status request and keeps the stored Rule; every other
    /// update writes the Rule directly.
    pub fn request_rule_update(&self, input: UpdateRuleInput) -> anyhow::Result<Rule> {
        let Some(status) = input.status.clone().and_then(requested_rule_status) else {
            return self.update_rule(input);
        };
        self.with_repository_publication(|held| {
            let record = crate::families::review_families::record(
                held,
                &input.scope_id,
                NodeType::Rule,
                &input.id,
            )?;
            let ReviewRecord::Rule(current) = &record else {
                return Err(crate::refusal::fault!("the Rule read gave another record kind"));
            };
            if current.status == rule_status(status) {
                return held.update_rule(input);
            }
            ensure_status_only(rule_fields_changed(&input))?;
            owner_matches(&record, input.declared_by.as_deref())?;
            check_etag(&record, input.expected_etag.as_deref())?;
            let mut requested = current.clone();
            requested.status = rule_status(status);
            requested.archived_in_commit = input.archived_in_commit.clone();
            requested
                .validate_transition(current)
                .map_err(|error| crate::refusal::refuse!(InvalidUpdate, "{error}"))?;
            let request = StatusRequest {
                status,
                archived_in_commit: input.archived_in_commit.clone(),
            };
            dry_run(held, |staged| {
                let path = shards::rules_path(&staged.layout, &input.scope_id);
                let id = input.id.clone();
                staged.permitting(&[&path], id.as_str(), |store| store.update_rule(input))
            })?;
            with_record_state(held, RecordStamp::ResolveFromRepository, |staged| {
                staged.open_status_request(&record, AUTHORING_ACTOR, request)
            })?;
            Ok(current.clone())
        })
    }

    /// Runs the `create-rule` operation. A new Rule with the active status
    /// has the review status, and its creation submission requests the
    /// active status.
    #[rule("rule_new_active_rule_waits_for_review")]
    pub fn request_rule_creation(&self, mut input: CreateRuleInput) -> anyhow::Result<Rule> {
        match input.status {
            RuleStatus::Archived => Err(crate::refusal::refuse!(
                InvalidUpdate,
                "a new Rule cannot have the archived status: create the Rule, then request \
                 the archive"
            )),
            RuleStatus::Active => {
                let path = shards::rules_path(&self.layout, &input.scope_id);
                let relative = path.strip_prefix(self.layout.root())?.to_owned();
                let id = input.id.clone();
                let stamp = self.current_record_stamp()?;
                self.with_repository_publication(|held| {
                    let direct = copy_rule_input(&input);
                    dry_run(held, |staged| {
                        let path = shards::rules_path(&staged.layout, &direct.scope_id);
                        let id = direct.id.clone();
                        staged.permitting(&[&path], id.as_str(), |store| store.write_rule(direct))
                    })?;
                    input.status = RuleStatus::Review;
                    with_record_state(held, RecordStamp::resolved(stamp), |staged| {
                        let staged_path = staged.layout.root().join(&relative);
                        let created = staged.permitting(&[&staged_path], id.as_str(), |store| {
                            store.write_rule(input)
                        })?;
                        staged.open_status_request(
                            &created.clone().into(),
                            AUTHORING_ACTOR,
                            StatusRequest {
                                status: RequestedStatus::Active,
                                archived_in_commit: None,
                            },
                        )?;
                        Ok(created)
                    })
                })
            }
            RuleStatus::Draft | RuleStatus::Review | RuleStatus::Deprecated => {
                self.create_rule(input)
            }
        }
    }

    /// Opens a review submission that requests a status, bound to the
    /// current revision of the record. The record keeps its status.
    #[rule("rule_status_change_waits_for_review")]
    pub(super) fn open_status_request(
        &self,
        record: &ReviewRecord,
        actor: &str,
        request: StatusRequest,
    ) -> anyhow::Result<()> {
        let revision = super::classifier::review_revision(record.kind(), record)?;
        let value = serde_json::to_value(record)?;
        let summary = ["statement", "position", "question", "title", "name"]
            .into_iter()
            .find_map(|field| value.get(field).and_then(serde_json::Value::as_str))
            .unwrap_or_else(|| record.id().as_str())
            .chars()
            .take(SUMMARY_CHARS)
            .collect::<String>();
        let input = SubmitRecordReview {
            scope_id: record.scope_id().clone(),
            actor: actor.to_owned(),
            record_kind: record.kind(),
            record_id: record.id().clone(),
            declared_by: value
                .get("declared_by")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            title: String::new(),
            summary,
            confidence: None,
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            builds_on: Vec::new(),
            expected_revision: Some(revision),
            revises: None,
        };
        check_request_size(&input)?;
        let path = shards::proposal_cards_path(&self.layout, record.scope_id());
        self.permitting(&[&path], "*", |store| {
            store.commit_submission_with(input, Some(request))
        })?;
        Ok(())
    }

    /// Sets the requested status on the reviewed record in the write of the
    /// approval.
    #[rule("rule_status_approval_sets_status")]
    pub(super) fn apply_status_request(
        &self,
        record: &ReviewRecord,
        request: &StatusRequest,
    ) -> anyhow::Result<()> {
        match record {
            ReviewRecord::Rule(rule) => {
                let path = shards::rules_path(&self.layout, &rule.scope_id);
                self.permitting(&[&path], rule.id.as_str(), |store| {
                    store.update_rule(rule_status_update(rule, request))
                })?;
                Ok(())
            }
            _ => Err(crate::refusal::fault!(
                "a status request applies only to a Rule or a Requirement"
            )),
        }
    }
}

/// A copy of a Rule creation input, for the dry run of the direct write.
fn copy_rule_input(input: &CreateRuleInput) -> CreateRuleInput {
    CreateRuleInput {
        scope_id: input.scope_id.clone(),
        id: input.id.clone(),
        name: input.name.clone(),
        description: input.description.clone(),
        requirement_ids: input.requirement_ids.clone(),
        resolution_ids: input.resolution_ids.clone(),
        statement: input.statement.clone(),
        status: input.status.clone(),
        archived_in_commit: input.archived_in_commit.clone(),
        severity: input.severity.clone(),
        source_document: input.source_document.clone(),
        source_section: input.source_section.clone(),
        origin_thread: input.origin_thread.clone(),
        origin_message: input.origin_message.clone(),
    }
}

fn rule_status_update(rule: &Rule, request: &StatusRequest) -> UpdateRuleInput {
    UpdateRuleInput {
        scope_id: rule.scope_id.clone(),
        id: rule.id.clone(),
        expected_etag: None,
        declared_by: rule.declared_by.clone(),
        name: None,
        description: None,
        statement: None,
        status: Some(rule_status(request.status)),
        severity: None,
        source_document: None,
        source_section: None,
        archived_in_commit: request.archived_in_commit.clone(),
        requirement_ids: None,
        resolution_ids: None,
        clear_fields: Vec::new(),
    }
}
