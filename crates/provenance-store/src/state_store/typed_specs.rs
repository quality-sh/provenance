use crate::write_error::{SourceFailure, WriteFailure};
mod adoption;
mod cascade;
mod decision;
mod deletion;
mod identity;
mod publication;
mod reconcile;
mod rule_addresses;
mod view;

use std::collections::{BTreeMap, BTreeSet};

use provenance_core::{
    DeclarationAddress, ImplementationBinding, Rule, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;

use super::requirement_reviews;
use super::{
    CascadedResource, ReconcileState, ReconciledResource, StateStore, TypedDeclarationKind,
    TypedFieldChange, TypedRuleInput, TypedSpecInput, TypedSpecResult,
};
pub use decision::{decide_typed_spec, TypedSpecDecision};
use decision::{CurrentTypedState, DesiredTypedIds};
pub(in crate::state_store) use rule_addresses::rule_address;
pub use view::TypedSpecView;

#[derive(Clone, Copy)]
pub(super) struct DesiredTypedGraph<'a> {
    pub(super) spec: &'a str,
    pub(super) owner: &'a str,
    pub(super) rules: &'a [TypedRuleInput],
    pub(super) rule_ids: &'a BTreeMap<DeclarationAddress, StableId>,
}

struct DesiredImplementations<'a> {
    spec: &'a str,
    owner: &'a str,
    rules: &'a [TypedRuleInput],
    rule_ids: &'a BTreeMap<DeclarationAddress, StableId>,
    adopted_rule_ids: &'a BTreeSet<String>,
    deleted_rule_ids: &'a BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum ReconcileMode {
    Plan,
    Apply,
}

impl StateStore {
    /// Reconciles one language-owned desired-state document with canonical state.
    ///
    /// Omitted owned records are deleted, while records from another owner
    /// remain untouched. Moves replace only active relationships owned by this
    /// spec, so applying one spec cannot take over another integration's or a
    /// human's records. The decision is computed again from the records read
    /// inside this publication section; a plan shown earlier does not count.
    #[rule("rule_typed_spec_apply_decides_inside_its_publication_section")]
    pub fn apply_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: TypedSpecInput,
    ) -> anyhow::Result<TypedSpecResult> {
        self.with_repository_publication(|held| {
            held.reconcile_typed_spec(scope_id, input, ReconcileMode::Apply)
        })
    }

    /// Calculates the exact typed-spec reconciliation without publishing it.
    pub fn plan_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: TypedSpecInput,
    ) -> anyhow::Result<TypedSpecResult> {
        self.with_repository_read(|held| {
            held.reconcile_typed_spec(scope_id, input, ReconcileMode::Plan)
        })
    }

    fn reconcile_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: TypedSpecInput,
        mode: ReconcileMode,
    ) -> anyhow::Result<TypedSpecResult> {
        self.validate_typed_spec(scope_id, &input)?;
        let view = self.typed_spec_view(scope_id)?;
        let decision = decide_typed_spec(scope_id, input, view)?;
        match mode {
            ReconcileMode::Plan => Ok(decision.result),
            ReconcileMode::Apply => decision.publish(self, scope_id),
        }
    }

    /// Puts the evidence of every Rule under a restated Requirement up for review.
    fn raise_requirement_reviews(
        &self,
        scope_id: &ScopeId,
        requirements: &[ReconciledResource],
        rules: &[ReconciledResource],
    ) -> anyhow::Result<()> {
        let changes = requirement_reviews::requirement_statement_changes(requirements);
        if changes.is_empty() {
            return Ok(());
        }
        let changed_at = requirement_reviews::now_millis()?;
        let mut reviews = Vec::new();
        for change in changes {
            let mut rule_ids = self.rule_ids_for_requirement(scope_id, &change.requirement_id)?;
            for rule in rules
                .iter()
                .filter(|rule| rule.parent.as_deref() == Some(change.requirement_key.as_str()))
            {
                if !rule_ids.contains(&rule.id) {
                    rule_ids.push(rule.id.clone());
                }
            }
            reviews.extend(rule_ids.into_iter().map(|rule_id| {
                requirement_reviews::RequirementReviewInput {
                    rule_id,
                    requirement_id: change.requirement_id.clone(),
                    field: change.field.clone(),
                    before: change.before.clone(),
                    after: change.after.clone(),
                    changed_at,
                }
            }));
        }
        self.record_requirement_reviews(scope_id, reviews)
    }

    fn validate_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: &TypedSpecInput,
    ) -> anyhow::Result<()> {
        if input.schema_version != SUPPORTED_SCHEMA_VERSION.0 {
            return Err(SourceFailure::wrap(
                WriteFailure::SchemaVersion,
                anyhow::anyhow!(
                    "typed spec schema_version must be {}",
                    SUPPORTED_SCHEMA_VERSION.0
                ),
            ));
        }
        crate::write_error::ensure!(
            InvalidDeclaration,
            !input.declared_by.trim().is_empty(),
            "declared_by must not be empty"
        );
        crate::write_error::ensure!(
            InvalidDeclaration,
            !input.spec.trim().is_empty(),
            "spec must not be empty"
        );
        crate::write_error::ensure!(
            InvalidDeclaration,
            self.manifest()?
                .scopes
                .iter()
                .any(|scope| scope.id.as_str() == scope_id.as_str()),
            "scope `{}` does not exist",
            scope_id.as_str()
        );
        Ok(())
    }
}

fn adopted_rule_ids(input: &TypedSpecInput) -> BTreeSet<String> {
    input
        .adopt_unowned
        .iter()
        .filter(|target| target.kind == TypedDeclarationKind::Rule)
        .map(|target| target.id.clone())
        .collect()
}

/// Assembles resources and diagnostics in decoded wire order; canonical
/// ordering applies only to kernel-authored documents.
#[rule("rule_rust_wire_order_is_preserved")]
fn spec_result(
    declared_by: String,
    resources: Vec<ReconciledResource>,
    cascade: Vec<CascadedResource>,
    implementation_bindings: Vec<provenance_core::ImplementationBinding>,
) -> TypedSpecResult {
    TypedSpecResult {
        declared_by,
        created: count_state(&resources, &cascade, ReconcileState::Created),
        updated: count_state(&resources, &cascade, ReconcileState::Updated),
        moved: count_state(&resources, &cascade, ReconcileState::Moved),
        deleted: count_state(&resources, &cascade, ReconcileState::Deleted),
        conflicts: count_state(&resources, &cascade, ReconcileState::Conflict),
        unchanged: count_state(&resources, &cascade, ReconcileState::Unchanged),
        resources,
        cascade,
        diagnostics: Vec::new(),
        implementation_bindings,
    }
}

fn replace_records<T: serde::de::DeserializeOwned + serde::Serialize>(
    store: &StateStore,
    path: &camino::Utf8Path,
    replacement: Vec<T>,
) -> anyhow::Result<()> {
    store.mutate_jsonl_records(path, |records| {
        *records = replacement;
        Ok(())
    })
}

fn count_state(
    resources: &[ReconciledResource],
    cascade: &[CascadedResource],
    state: ReconcileState,
) -> usize {
    resources
        .iter()
        .map(|resource| resource.state)
        .chain(cascade.iter().map(|resource| resource.state))
        .filter(|candidate| *candidate == state)
        .count()
}

fn all_resources(
    sources: &[ReconciledResource],
    requirements: &[ReconciledResource],
    rules: &[ReconciledResource],
) -> Vec<ReconciledResource> {
    [sources, requirements, rules].concat()
}

fn attach_implementation_changes(
    resources: &mut [ReconciledResource],
    changes: &[(StableId, TypedFieldChange)],
) {
    for resource in resources.iter_mut() {
        let Some((_, change)) = changes.iter().find(|(id, _)| id == &resource.id) else {
            continue;
        };
        if resource.state == ReconcileState::Unchanged {
            resource.state = ReconcileState::Updated;
        }
        resource.changes.push(change.clone());
    }
}

fn reconcile_implementations(
    current: &[ImplementationBinding],
    scope_id: &ScopeId,
    desired: &DesiredImplementations<'_>,
    canonical_rules: &[Rule],
    rule_resources: &mut [ReconciledResource],
) -> anyhow::Result<super::implementation_bindings::Reconciliation> {
    let graph = DesiredTypedGraph {
        spec: desired.spec,
        owner: desired.owner,
        rules: desired.rules,
        rule_ids: desired.rule_ids,
    };
    let mut reconciliation = super::implementation_bindings::reconcile(
        current,
        scope_id,
        graph,
        canonical_rules,
        desired.adopted_rule_ids,
    )?;
    reconciliation
        .records
        .retain(|record| !desired.deleted_rule_ids.contains(record.rule_id.as_str()));
    attach_implementation_changes(rule_resources, &reconciliation.changes);
    Ok(reconciliation)
}
