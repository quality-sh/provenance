use super::{guard, journal};
use crate::{
    canonical_digest,
    publication::with_staged_state,
    shards,
    state_store::{CreateRequirementInput, StateStore},
};
use provenance_core::{
    review::{ReviewEntry, SaveOutcome, REVIEW_SCHEMA_VERSION},
    threads::DiscussionOrigin,
    Requirement,
};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReviewRequirement {
    pub request_id: provenance_core::StableId,
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
        mut input: CreateReviewRequirement,
    ) -> anyhow::Result<ReviewEntry> {
        let digest = normalize(&mut input)?;
        self.with_repository_publication(|| {
            let scope = &input.create.scope_id;
            if let Some(receipt) = self.creation_receipt(&input, &digest)? {
                return Ok(receipt);
            }
            anyhow::ensure!(
                !self
                    .list_requirements(scope)?
                    .iter()
                    .any(|r| r.id == input.create.id),
                "Requirement already exists"
            );
            self.validated_review_entries(scope)?;
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
            with_staged_state(&self.layout, false, |layout| {
                guard::with_writer(
                    &shards::requirements_path(layout, &scope),
                    id.as_str(),
                    || Self::new(layout.clone()).commit_creation(input, digest),
                )
            })
        })
    }

    /// Reports absence only after recovery, scope checks, and current owner checks.
    pub fn requirement_creation_receipt(
        &self,
        mut input: CreateReviewRequirement,
    ) -> anyhow::Result<Option<ReviewEntry>> {
        let digest = normalize(&mut input)?;
        self.with_repository_publication(|| self.creation_receipt(&input, &digest))
    }

    fn creation_receipt(
        &self,
        input: &CreateReviewRequirement,
        digest: &str,
    ) -> anyhow::Result<Option<ReviewEntry>> {
        let scope = &input.create.scope_id;
        anyhow::ensure!(
            self.manifest()?.scopes.iter().any(|s| s.id == *scope),
            "review scope is not in the manifest"
        );
        let records = self.list_requirements(scope)?;
        let current = records
            .iter()
            .filter(|r| r.id == input.create.id)
            .collect::<Vec<_>>();
        anyhow::ensure!(current.len() <= 1, "duplicate Requirement identity");
        if let Some(record) = current.first() {
            anyhow::ensure!(record.scope_id == *scope, "Requirement scope mismatch");
            super::owner_matches(record, None)?;
        }
        let path = journal::entry_path(&self.layout, scope, &input.request_id);
        if !path.try_exists()? {
            return Ok(None);
        }
        let entry = journal::read_entry(&self.layout, &path)?;
        anyhow::ensure!(
            !current.is_empty()
                && entry.scope_id == *scope
                && entry.requirement_id == input.create.id
                && entry.request_id == input.request_id
                && entry.actor == input.actor
                && entry.intent_digest == digest,
            "review request ID was reused with different intent"
        );
        Ok(Some(entry))
    }

    fn commit_creation(
        &self,
        input: CreateReviewRequirement,
        intent_digest: String,
    ) -> anyhow::Result<ReviewEntry> {
        let scope = input.create.scope_id.clone();
        let created = self.create_requirement(input.create)?;
        let path = shards::requirements_path(&self.layout, &scope);
        let after = self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
            let record = records.iter_mut().find(|r| r.id == created.id).unwrap();
            record.schema_version = REVIEW_SCHEMA_VERSION;
            Ok(record.clone())
        })?;
        self.validate_graph_scope(&scope)?;
        self.enroll_review_manifest()?;
        let id = journal::new_id();
        let entry = ReviewEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            scope_id: scope.clone(),
            requirement_id: after.id.clone(),
            sequence: 1,
            predecessor: None,
            prior_revision: None,
            revision: journal::new_id(),
            before: None,
            after: journal::snapshot(&self.layout, &after)?,
            changed_fields: serde_json::to_value(&after)?
                .as_object()
                .unwrap()
                .keys()
                .filter(|k| k.as_str() != "schema_version")
                .cloned()
                .collect(),
            actor: input.actor,
            request_id: input.request_id,
            intent_digest,
            etag: journal::etag(&after, Some(&id))?,
            id,
            outcome: SaveOutcome::Created,
            origin: input.origin,
        };
        anyhow::ensure!(
            serde_json::to_vec(&entry)?.len() as u64 <= journal::ENTRY_BYTES,
            "review receipt exceeds the entry byte budget"
        );
        journal::write_new(
            &journal::entry_path(&self.layout, &scope, &entry.request_id),
            &entry,
        )?;
        Ok(entry)
    }
}

fn normalize(input: &mut CreateReviewRequirement) -> anyhow::Result<String> {
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
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(input)?,
    ))
}
