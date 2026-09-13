use crate::write_error::{SourceFailure, WriteFailure};
mod adoption;
mod cascade;
mod deletion;
mod identity;
mod publication;
mod reconcile;
mod rule_addresses;

use std::collections::{BTreeMap, BTreeSet};

use provenance_core::{
    DeclarationAddress, ImplementationBinding, Requirement, Rule, ScopeId, Source, StableId,
    SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;

use super::requirement_reviews;
use super::{
    CascadedResource, ReconcileState, ReconciledResource, StateStore, TypedDeclarationKind,
    TypedFieldChange, TypedRuleInput, TypedSpecInput, TypedSpecResult,
};
use identity::{
    declaration_ids, normalize_rule_relationships, owned_declaration_ids, requirement_identity,
    rule_declaration_ids, source_identity, validate_references,
};
use reconcile::{
    ensure_acyclic, ensure_resolutions_exist, reconcile_requirements, reconcile_rules,
    reconcile_sources,
};
pub(in crate::state_store) use rule_addresses::rule_address;

struct CurrentTypedState {
    sources: Vec<Source>,
    requirements: Vec<Requirement>,
    rules: Vec<Rule>,
    implementation_bindings: Vec<ImplementationBinding>,
    source_addresses: BTreeMap<DeclarationAddress, StableId>,
    requirement_addresses: BTreeMap<DeclarationAddress, StableId>,
    rule_addresses: BTreeMap<DeclarationAddress, StableId>,
}

struct DesiredTypedIds {
    sources: BTreeMap<String, StableId>,
    requirements: BTreeMap<String, StableId>,
    rules: BTreeMap<DeclarationAddress, StableId>,
}

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

/// Runs the kernel checkers and id resolution at the pinned pipeline
/// points, so a document keeps its pre-extraction accept-or-reject
/// result and, for a multi-defect document, its first error.
#[rule("rule_rust_wire_acceptance_is_stable")]
#[rule("rule_rust_wire_first_error_is_stable")]
fn desired_typed_ids(
    input: &TypedSpecInput,
    current: &CurrentTypedState,
) -> anyhow::Result<DesiredTypedIds> {
    let sources = declaration_ids(
        "source",
        &input.declared_by,
        &input.spec,
        input.sources.iter().map(source_identity),
        &current.source_addresses,
    )?;
    let requirements = declaration_ids(
        "requirement",
        &input.declared_by,
        &input.spec,
        input.requirements.iter().map(requirement_identity),
        &current.requirement_addresses,
    )?;
    let rules = rule_declaration_ids(
        &input.declared_by,
        &input.spec,
        &input.rules,
        &current.rule_addresses,
    )?;
    validate_references(
        &input.sources,
        &input.requirements,
        &input.rules,
        |key| sources.contains_key(key),
        |key| requirements.contains_key(key),
    )
    .map_err(|error| SourceFailure::wrap(WriteFailure::MissingReference, error))?;
    Ok(DesiredTypedIds {
        sources,
        requirements,
        rules,
    })
}

impl StateStore {
    /// Reconciles one language-owned desired-state document with canonical state.
    ///
    /// Omitted owned records are deleted, while records from another owner
    /// remain untouched. Moves replace only active relationships owned by this
    /// spec, so applying one spec cannot take over another integration's or a
    /// human's records.
    pub fn apply_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: TypedSpecInput,
    ) -> anyhow::Result<TypedSpecResult> {
        self.with_repository_publication(|| {
            self.reconcile_typed_spec(scope_id, input, ReconcileMode::Apply)
        })
    }

    /// Calculates the exact typed-spec reconciliation without publishing it.
    pub fn plan_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: TypedSpecInput,
    ) -> anyhow::Result<TypedSpecResult> {
        self.with_repository_read(|| {
            self.reconcile_typed_spec(scope_id, input, ReconcileMode::Plan)
        })
    }

    fn reconcile_typed_spec(
        &self,
        scope_id: &ScopeId,
        input: TypedSpecInput,
        mode: ReconcileMode,
    ) -> anyhow::Result<TypedSpecResult> {
        let (input, current) = self.prepare_typed_spec(scope_id, input)?;
        let ids = desired_typed_ids(&input, &current)
            .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidDeclaration, error))?;
        let ownership = adoption::decide(scope_id, &input, &current, &ids)?;
        if !ownership.conflicts().is_empty() {
            if matches!(mode, ReconcileMode::Apply) {
                ownership.refuse()?;
                unreachable!("ownership decision must reject reported conflicts");
            }
            return Ok(spec_result(
                input.declared_by,
                ownership.into_conflicts(),
                Vec::new(),
                Vec::new(),
            ));
        }
        let adopted_rule_ids = adopted_rule_ids(&input);
        let rule_relationships = input.rules.clone();
        let spec = input.spec;
        let (mut sources, source_resources) = reconcile_sources(
            current.sources,
            &spec,
            scope_id,
            &input.declared_by,
            input.sources,
            &ids.sources,
        )?;
        let (mut requirements, requirement_resources) = reconcile_requirements(
            current.requirements,
            &spec,
            scope_id,
            &input.declared_by,
            input.requirements,
            &ids.requirements,
            &ids.sources,
        )?;
        let (mut rules, mut rule_resources) = reconcile_rules(
            current.rules,
            &spec,
            scope_id,
            &input.declared_by,
            input.rules,
            &ids.rules,
            &ids.requirements,
        )?;
        let deleted_resources =
            all_resources(&source_resources, &requirement_resources, &rule_resources);
        let cascade = cascade::Cascade::prepare(
            self,
            scope_id,
            &deleted_resources,
            &mut sources,
            &mut requirements,
            &mut rules,
        )?;
        ensure_resolutions_exist(self, scope_id, &requirements, &rules)?;
        ensure_acyclic(&requirements)?;
        let implementation_reconciliation = reconcile_implementations(
            self,
            scope_id,
            &DesiredImplementations {
                spec: &spec,
                owner: &input.declared_by,
                rules: &rule_relationships,
                rule_ids: &ids.rules,
                adopted_rule_ids: &adopted_rule_ids,
                deleted_rule_ids: &cascade.rules,
            },
            &rules,
            &mut rule_resources,
        )?;
        let mut resources =
            all_resources(&source_resources, &requirement_resources, &rule_resources);
        let cascade_resources = cascade.report(&mut resources);
        let mut result = spec_result(
            input.declared_by,
            resources,
            cascade_resources,
            implementation_reconciliation.active,
        );
        self.analyze_typed_result(&mut result, &requirements, &rules);

        if matches!(mode, ReconcileMode::Apply) {
            publication::Replacement {
                sources,
                requirements,
                rules,
                implementations: implementation_reconciliation.records,
                cascade,
            }
            .publish(
                self,
                scope_id,
                &result,
                &requirement_resources,
                &rule_resources,
            )?;
        }

        Ok(result)
    }

    fn analyze_typed_result(
        &self,
        result: &mut TypedSpecResult,
        requirements: &[Requirement],
        rules: &[Rule],
    ) {
        let dictionary = crate::dictionary_reference::load_project_dictionary(&self.layout);
        result.diagnostics = super::typed_statement_policy::analyze_typed_statements(
            &result.resources,
            requirements,
            rules,
            dictionary.as_ref(),
        );
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

    fn current_typed_state(
        &self,
        scope_id: &ScopeId,
        owner: &str,
    ) -> anyhow::Result<CurrentTypedState> {
        let sources = self.list_sources(scope_id)?;
        let requirements = self.list_requirements(scope_id)?;
        let rules = self.list_rules(scope_id)?;
        let implementation_bindings = self.list_implementation_bindings(scope_id)?;
        let source_addresses = owned_declaration_ids(owner, &sources, |record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        })?;
        let requirement_addresses = owned_declaration_ids(owner, &requirements, |record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        })?;
        let rule_addresses = owned_declaration_ids(owner, &rules, |record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        })?;
        Ok(CurrentTypedState {
            sources,
            requirements,
            rules,
            implementation_bindings,
            source_addresses,
            requirement_addresses,
            rule_addresses,
        })
    }

    fn prepare_typed_spec(
        &self,
        scope_id: &ScopeId,
        mut input: TypedSpecInput,
    ) -> anyhow::Result<(TypedSpecInput, CurrentTypedState)> {
        self.validate_typed_spec(scope_id, &input)?;
        normalize_rule_relationships(&mut input.rules)
            .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidDeclaration, error))?;
        let current = self.current_typed_state(scope_id, &input.declared_by)?;
        Ok((input, current))
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
    store: &StateStore,
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
        store,
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
