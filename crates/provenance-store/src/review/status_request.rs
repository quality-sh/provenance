//! Status requests: a change of the status of a Rule or a Requirement to
//! active or archived waits for a reviewer. The request opens a review
//! submission that names the requested status, and an approval of that
//! submission sets the status.

use super::{
    automatic_submission::record_submission, input::RequirementRelations, owner_matches,
    publication::with_record_state, write_actor::AUTHORING_ACTOR, SaveRequirement,
};
use crate::{
    shards,
    state_store::{
        CreateRuleInput, RecordStamp, StateStore, UpdateRequirementInput, UpdateRuleInput,
    },
};
use provenance_core::{
    review::{RequestedStatus, ReviewRecord, StatusRequest},
    DispositionDecision, NodeType, RecordRevisionBinding, Requirement, Rule, RuleStatus,
};
use provenance_macros::rule;

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

/// Each field of a Rule update other than the status and its guards. Each
/// field is named, so a new field of the input does not compile until it is
/// sorted here.
const fn rule_fields_changed(input: &UpdateRuleInput) -> bool {
    let UpdateRuleInput {
        scope_id: _,
        id: _,
        expected_etag: _,
        declared_by: _,
        status: _,
        archived_in_commit: _,
        name,
        description,
        statement,
        severity,
        source_document,
        source_section,
        requirement_ids,
        resolution_ids,
        clear_fields,
    } = input;
    name.is_some()
        || description.is_some()
        || statement.is_some()
        || severity.is_some()
        || source_document.is_some()
        || source_section.is_some()
        || requirement_ids.is_some()
        || resolution_ids.is_some()
        || !clear_fields.is_empty()
}

/// Each field of a Requirement save other than the status and its guards,
/// named as in [`rule_fields_changed`].
fn requirement_fields_changed(input: &SaveRequirement) -> bool {
    let SaveRequirement {
        actor: _,
        expected_etag: _,
        update,
        relationships,
    } = input;
    let UpdateRequirementInput {
        scope_id: _,
        id: _,
        expected_etag: _,
        declared_by: _,
        status: _,
        archived_in_commit: _,
        statement,
        description,
        clear_fields,
    } = update;
    let relations_changed = relationships.as_ref().is_some_and(|relations| {
        let RequirementRelations {
            refines,
            depends_on,
            supersedes,
            spawned_by,
            cites,
        } = relations;
        refines.is_some()
            || depends_on.is_some()
            || supersedes.is_some()
            || spawned_by.is_some()
            || cites.is_some()
    });
    statement.is_some() || description.is_some() || !clear_fields.is_empty() || relations_changed
}

/// The status request of a Requirement save, when the save changes the
/// status to active or archived. A save that requests a status and changes
/// another field is refused.
pub(super) fn requirement_status_request(
    input: &SaveRequirement,
    current: &Requirement,
) -> anyhow::Result<Option<StatusRequest>> {
    let Some(status) = input
        .update
        .status
        .as_ref()
        .and_then(RequestedStatus::of_requirement)
    else {
        return Ok(None);
    };
    if current.status == status.requirement_status() {
        return Ok(None);
    }
    ensure_status_only(requirement_fields_changed(input))?;
    Ok(Some(StatusRequest {
        status,
        archived_in_commit: input.update.archived_in_commit.clone(),
    }))
}

impl StateStore {
    /// Writes a Rule update. A change of the status to active or archived
    /// opens a status request and keeps the stored Rule; every other update
    /// writes the Rule directly.
    pub fn update_rule(&self, input: UpdateRuleInput) -> anyhow::Result<Rule> {
        let Some(status) = input.status.as_ref().and_then(RequestedStatus::of_rule) else {
            return self.prepare_rule_update(input);
        };
        self.with_repository_publication(|held| {
            let record = crate::families::review_families::record(
                held,
                &input.scope_id,
                NodeType::Rule,
                &input.id,
            )?;
            let ReviewRecord::Rule(current) = &record else {
                return Err(crate::refusal::fault!(
                    "the Rule read gave another record kind"
                ));
            };
            if current.status == status.rule_status() {
                return held.prepare_rule_update(input);
            }
            owner_matches(&record, input.declared_by.as_deref())?;
            ensure_status_only(rule_fields_changed(&input))?;
            if let Some(expected) = input.expected_etag.as_deref() {
                super::save::ensure_etag(&record, expected)?;
            }
            let mut requested = current.clone();
            requested.status = status.rule_status();
            requested
                .archived_in_commit
                .clone_from(&input.archived_in_commit);
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
                staged.permitting(&[&path], id.as_str(), |store| {
                    store.prepare_rule_update(input)
                })
            })?;
            with_record_state(held, RecordStamp::ResolveFromRepository, |staged| {
                staged.open_status_request(&record, AUTHORING_ACTOR, request)
            })?;
            Ok(current.clone())
        })
    }

    /// Creates a Rule and opens its review submission. A new Rule with the
    /// active status has the review status, and its creation submission
    /// requests the active status.
    #[rule("rule_new_active_rule_waits_for_review")]
    pub fn create_rule(&self, mut input: CreateRuleInput) -> anyhow::Result<Rule> {
        let path = shards::rules_path(&self.layout, &input.scope_id);
        let id = input.id.clone();
        let request = match RequestedStatus::of_rule(&input.status) {
            None => None,
            Some(RequestedStatus::Archived) => {
                return Err(crate::refusal::refuse!(
                    InvalidUpdate,
                    "a new Rule cannot have the archived status: create the Rule, then request \
                     the archive"
                ));
            }
            Some(status) => Some(StatusRequest {
                status,
                archived_in_commit: None,
            }),
        };
        self.with_repository_publication(|held| {
            if request.is_some() {
                let direct = input.clone();
                dry_run(held, |staged| {
                    let path = shards::rules_path(&staged.layout, &direct.scope_id);
                    staged.permitting(&[&path], id.as_str(), |store| store.write_rule(direct))
                })?;
                input.status = RuleStatus::Review;
            }
            held.create_native_record_with(&path, &id, request, |store| store.write_rule(input))
        })
    }

    /// Opens the status request of a Requirement save after the checks of
    /// the direct save, and keeps the stored Requirement.
    pub(super) fn request_requirement_status(
        &self,
        input: SaveRequirement,
        current: &Requirement,
        request: StatusRequest,
    ) -> anyhow::Result<()> {
        let record = ReviewRecord::from(current.clone());
        super::save::ensure_etag(&record, &input.expected_etag)?;
        let mut requested = current.clone();
        requested.status = request.status.requirement_status();
        requested
            .archived_in_commit
            .clone_from(&request.archived_in_commit);
        requested
            .validate_transition(current)
            .map_err(|error| crate::refusal::refuse!(InvalidUpdate, "{error}"))?;
        let actor = input.actor.clone();
        dry_run(self, |staged| {
            let path = shards::requirements_path(&staged.layout, &current.scope_id);
            staged.permitting(&[&path], current.id.as_str(), |store| {
                store.commit_requirement(input, current, None, None)
            })
        })?;
        with_record_state(self, RecordStamp::ResolveFromRepository, |staged| {
            staged.open_status_request(&record, &actor, request)
        })
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
        let input = record_submission(record, actor, &revision)?;
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
                    store.prepare_rule_update(rule_status_update(rule, request))
                })?;
                Ok(())
            }
            ReviewRecord::Requirement(requirement) => {
                let path = shards::requirements_path(&self.layout, &requirement.scope_id);
                self.permitting(&[&path], requirement.id.as_str(), |store| {
                    store.apply_requirement_update(requirement_status_update(requirement, request))
                })?;
                self.validate_graph_scope(&requirement.scope_id)?;
                Ok(())
            }
            _ => Err(crate::refusal::fault!(
                "a status request applies only to a Rule or a Requirement"
            )),
        }
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
        status: Some(request.status.rule_status()),
        severity: None,
        source_document: None,
        source_section: None,
        archived_in_commit: request.archived_in_commit.clone(),
        requirement_ids: None,
        resolution_ids: None,
        clear_fields: Vec::new(),
    }
}

fn requirement_status_update(
    requirement: &Requirement,
    request: &StatusRequest,
) -> UpdateRequirementInput {
    UpdateRequirementInput {
        scope_id: requirement.scope_id.clone(),
        id: requirement.id.clone(),
        expected_etag: None,
        declared_by: requirement.declared_by.clone(),
        statement: None,
        description: None,
        status: Some(request.status.requirement_status()),
        archived_in_commit: request.archived_in_commit.clone(),
        clear_fields: Vec::new(),
    }
}
