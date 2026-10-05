use super::{classifier, guard};
use crate::{
    review::publication::with_record_state,
    shards,
    state_store::{CreateRequirementInput, StateStore},
};
use provenance_core::{
    review::RequirementEditState, threads::DiscussionOrigin, NodeType, Requirement,
};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReviewRequirement {
    pub actor: String,
    pub create: CreateRequirementInput,
    pub origin: Option<DiscussionOrigin>,
}

impl StateStore {
    /// Creates a Requirement from a Discussion comment and keeps that comment
    /// as the created record's origin. The origin Thread and Message must
    /// exist in an addressed Discussion before the record is published.
    #[rule("rule_comment_created_record_retains_discussion_origin")]
    pub fn create_review_requirement(
        &self,
        input: CreateReviewRequirement,
    ) -> anyhow::Result<RequirementEditState> {
        self.create_review_requirement_with(input, |_, created| {
            Self::record_edit_state_for_record(&created.into())
        })
    }

    pub(crate) fn create_review_requirement_resource(
        &self,
        input: CreateReviewRequirement,
    ) -> anyhow::Result<super::RequirementResourceSnapshot> {
        self.create_review_requirement_with(input, |store, created| {
            store.requirement_resource_snapshot_unlocked(&created.scope_id, &created.id)
        })
    }

    fn create_review_requirement_with<R>(
        &self,
        mut input: CreateReviewRequirement,
        complete: impl FnOnce(&Self, Requirement) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        normalize(&mut input)?;
        self.with_repository_publication(|| {
            let scope = &input.create.scope_id;
            anyhow::ensure!(
                self.manifest()?.scopes.iter().any(|s| s.id == *scope),
                "review scope is not in the manifest"
            );
            crate::write_error::ensure!(
                AlreadyExists,
                !self
                    .list_requirements(scope)?
                    .iter()
                    .any(|r| r.id == input.create.id),
                "Requirement already exists"
            );
            if let Some(origin) = &input.origin {
                self.validate_discussion_origin(scope, origin)?;
            }
            anyhow::ensure!(
                input.origin.is_some()
                    || !self.origin_requires_review(scope, input.create.origin_message.as_ref())?,
                "addressed creation requires Discussion origin"
            );
            self.validate_requirement_origin(
                scope,
                input.create.origin_thread.as_ref(),
                input.create.origin_message.as_ref(),
            )?;
            let scope = scope.clone();
            let id = input.create.id.clone();
            let stamp = self.current_record_stamp()?;
            with_record_state(&self.layout, |layout| {
                guard::with_writer(
                    &shards::requirements_path(layout, &scope),
                    id.as_str(),
                    || {
                        let staged = Self::staged(layout.clone(), stamp);
                        let created = staged.commit_creation(input)?;
                        complete(&staged, created)
                    },
                )
            })
        })
    }

    fn commit_creation(&self, input: CreateReviewRequirement) -> anyhow::Result<Requirement> {
        let scope = input.create.scope_id.clone();
        let after = self.write_requirement(input.create)?;
        self.validate_graph_scope(&scope)?;
        let revision = classifier::review_revision(NodeType::Requirement, &after)?;
        if let Some(origin) = &input.origin {
            self.add_discussion_outcome(
                &scope,
                origin,
                NodeType::Requirement,
                &after.id,
                &revision,
            )?;
        }
        self.commit_automatic_submission(&after, &input.actor, &revision)?;
        Ok(after)
    }
}

fn normalize(input: &mut CreateReviewRequirement) -> anyhow::Result<()> {
    anyhow::ensure!(
        !input.actor.trim().is_empty(),
        "invalid review request identity"
    );
    anyhow::ensure!(
        serde_json::to_vec(&input)?.len() <= 1_048_576,
        "review request exceeds the operation byte budget"
    );
    for ids in [&mut input.create.depends_on, &mut input.create.supersedes] {
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        ids.dedup();
    }
    if let Some(origin) = &input.origin {
        anyhow::ensure!(
            input
                .create
                .origin_thread
                .as_ref()
                .is_none_or(|id| id == &origin.thread_id)
                && input
                    .create
                    .origin_message
                    .as_ref()
                    .is_none_or(|id| id == &origin.message_id),
            "creation origin differs from Discussion origin"
        );
        input.create.origin_thread = Some(origin.thread_id.clone());
        input.create.origin_message = Some(origin.message_id.clone());
    }
    Ok(())
}
