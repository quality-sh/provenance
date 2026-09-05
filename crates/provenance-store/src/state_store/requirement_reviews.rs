use provenance_core::{RequirementReview, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION};
use sha2::{Digest, Sha256};

use super::{ReconciledResource, StateStore, TypedResourceKind};
use crate::shards;

/// One restated Requirement obligation drawn from a reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementStatementChange {
    pub requirement_id: StableId,
    pub requirement_key: String,
    pub field: String,
    pub before: String,
    pub after: String,
}

/// Reads the Requirement restatements out of one reconciliation.
///
/// Only the statement counts. It carries the obligation, so changing it is
/// what asks the evidence to be looked at again.
pub fn requirement_statement_changes(
    resources: &[ReconciledResource],
) -> Vec<RequirementStatementChange> {
    resources
        .iter()
        .filter(|resource| resource.kind == TypedResourceKind::Requirement)
        .flat_map(|resource| {
            resource
                .changes
                .iter()
                .filter(|change| change.field == "statement")
                .filter_map(|change| {
                    Some(RequirementStatementChange {
                        requirement_id: resource.id.clone(),
                        requirement_key: resource.key.clone(),
                        field: change.field.clone(),
                        before: change.before.as_str()?.to_string(),
                        after: change.after.as_str()?.to_string(),
                    })
                })
        })
        .collect()
}

/// One Rule whose evidence a restated Requirement puts up for review.
pub struct RequirementReviewInput {
    pub rule_id: StableId,
    pub requirement_id: StableId,
    pub field: String,
    pub before: String,
    pub after: String,
    pub changed_at: i64,
}

impl StateStore {
    /// Records why each Rule's evidence needs review after a Requirement changed.
    ///
    /// Re-applying the same change keeps the record already on file, so a
    /// review a verification run already cleared does not reopen itself.
    pub(crate) fn record_requirement_reviews(
        &self,
        scope: &ScopeId,
        reviews: Vec<RequirementReviewInput>,
    ) -> anyhow::Result<()> {
        if reviews.is_empty() {
            return Ok(());
        }
        let desired = reviews
            .into_iter()
            .map(|review| review.into_record(scope))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let path = shards::requirement_reviews_path(&self.layout, scope);
        self.mutate_jsonl_records(&path, |records: &mut Vec<RequirementReview>| {
            for record in desired {
                if !records.iter().any(|existing| existing.id == record.id) {
                    records.push(record);
                }
            }
            records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
            Ok(())
        })
    }

    /// Clears the reviews a Rule's own fresh verification run answers.
    ///
    /// The reason stays on the record. Only reviews raised before the run
    /// clear, because an earlier run cannot vouch for a later change.
    pub(crate) fn clear_requirement_reviews(
        &self,
        scope: &ScopeId,
        rule_id: &StableId,
        run_id: &StableId,
        ran_at: i64,
    ) -> anyhow::Result<()> {
        let path = shards::requirement_reviews_path(&self.layout, scope);
        if !path.exists() {
            return Ok(());
        }
        self.mutate_jsonl_records(&path, |records: &mut Vec<RequirementReview>| {
            for record in records.iter_mut().filter(|record| {
                record.rule_id == *rule_id
                    && record.cleared_at.is_none()
                    && record.changed_at <= ran_at
            }) {
                record.cleared_at = Some(ran_at);
                record.cleared_by_run = Some(run_id.clone());
            }
            Ok(())
        })
    }

    /// Every review ever raised in this scope, cleared ones included.
    pub fn list_requirement_reviews(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<RequirementReview>> {
        let path = shards::requirement_reviews_path(&self.layout, scope);
        crate::test_probes::record_read(&path);
        if !path.exists() {
            return Ok(Vec::new());
        }
        std::fs::read_to_string(path)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(Into::into))
            .collect()
    }

    /// The Rules one Requirement currently produces.
    pub fn rule_ids_for_requirement(
        &self,
        scope: &ScopeId,
        requirement_id: &StableId,
    ) -> anyhow::Result<Vec<StableId>> {
        Ok(self
            .list_rules(scope)?
            .into_iter()
            .filter(|rule| rule.requirement_ids.contains(requirement_id))
            .map(|rule| rule.id)
            .collect())
    }

    /// The reviews still waiting on a verification run.
    pub fn open_requirement_reviews(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<RequirementReview>> {
        Ok(self
            .list_requirement_reviews(scope)?
            .into_iter()
            .filter(|record| record.cleared_at.is_none())
            .collect())
    }
}

impl RequirementReviewInput {
    fn into_record(self, scope: &ScopeId) -> anyhow::Result<RequirementReview> {
        let id = review_id(&self)?;
        Ok(RequirementReview {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope.clone(),
            id,
            rule_id: self.rule_id,
            requirement_id: self.requirement_id,
            field: self.field,
            before: self.before,
            after: self.after,
            changed_at: self.changed_at,
            cleared_at: None,
            cleared_by_run: None,
        })
    }
}

/// Milliseconds since the Unix epoch.
pub fn now_millis() -> anyhow::Result<i64> {
    let duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    i64::try_from(duration.as_millis()).map_err(Into::into)
}

/// Identifies one review by the Rule and the exact change that raised it.
fn review_id(input: &RequirementReviewInput) -> anyhow::Result<StableId> {
    let identity = format!(
        "{}\0{}\0{}\0{}",
        input.rule_id.as_str(),
        input.requirement_id.as_str(),
        input.field,
        input.after
    );
    let digest = format!("{:x}", Sha256::digest(identity.as_bytes()));
    StableId::new(format!("requirement_review_{}", &digest[..20]))
}
