//! The typed-spec decision: a function of one input and one record view.

use std::collections::BTreeMap;

use provenance_core::{
    DeclarationAddress, ImplementationBinding, Requirement, Rule, ScopeId, Source, StableId,
};
use provenance_macros::rule;

use super::adoption::{self, OwnershipDecision};
use super::identity::{
    declaration_ids, normalize_rule_relationships, owned_declaration_ids, requirement_identity,
    rule_declaration_ids, source_identity, validate_references,
};
use super::reconcile::{ensure_acyclic, ensure_resolutions_exist, reconcile};
use super::view::TypedSpecView;
use super::{
    adopted_rule_ids, all_resources, cascade, publication, reconcile_implementations, spec_result,
    DesiredImplementations,
};
use crate::state_store::{ReconciledResource, StateStore, TypedSpecInput, TypedSpecResult};
use crate::write_error::{SourceFailure, WriteFailure};

pub(super) struct CurrentTypedState {
    pub(super) sources: Vec<Source>,
    pub(super) requirements: Vec<Requirement>,
    pub(super) rules: Vec<Rule>,
    pub(super) implementation_bindings: Vec<ImplementationBinding>,
    pub(super) source_addresses: BTreeMap<DeclarationAddress, StableId>,
    pub(super) requirement_addresses: BTreeMap<DeclarationAddress, StableId>,
    pub(super) rule_addresses: BTreeMap<DeclarationAddress, StableId>,
}

pub(super) struct DesiredTypedIds {
    pub(super) sources: BTreeMap<String, StableId>,
    pub(super) requirements: BTreeMap<String, StableId>,
    pub(super) rules: BTreeMap<DeclarationAddress, StableId>,
}

/// The reconciliation of one typed spec and the records it would publish.
///
/// Only the store publishes a decision, and only one it computed inside its
/// own publication section.
pub struct TypedSpecDecision {
    pub result: TypedSpecResult,
    outcome: Outcome,
}

enum Outcome {
    Conflicts(OwnershipDecision),
    Replace {
        replacement: Box<publication::Replacement>,
        requirement_resources: Vec<ReconciledResource>,
        rule_resources: Vec<ReconciledResource>,
    },
}

impl TypedSpecDecision {
    /// Publishes the replacement, or refuses with the ownership conflicts.
    pub(super) fn publish(
        self,
        store: &StateStore,
        scope_id: &ScopeId,
    ) -> anyhow::Result<TypedSpecResult> {
        match self.outcome {
            Outcome::Conflicts(ownership) => {
                ownership.refuse()?;
                unreachable!("ownership decision must reject reported conflicts");
            }
            Outcome::Replace {
                replacement,
                requirement_resources,
                rule_resources,
            } => {
                replacement.publish(
                    store,
                    scope_id,
                    &self.result,
                    &requirement_resources,
                    &rule_resources,
                )?;
                Ok(self.result)
            }
        }
    }
}

/// Decides what one typed spec does to the records in `view`.
///
/// The decision reads no store, layout or lock. The input must have passed
/// the store's schema, owner and scope checks.
#[rule("rule_typed_spec_decision_reads_only_supplied_records")]
pub fn decide_typed_spec(
    scope_id: &ScopeId,
    mut input: TypedSpecInput,
    view: &TypedSpecView,
) -> anyhow::Result<TypedSpecDecision> {
    normalize_rule_relationships(&mut input.rules)
        .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidDeclaration, error))?;
    let current = CurrentTypedState::new(&input.declared_by, view)?;
    let ids = desired_typed_ids(&input, &current)
        .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidDeclaration, error))?;
    let ownership = adoption::decide(scope_id, &input, &current, &ids)?;
    if !ownership.conflicts().is_empty() {
        return Ok(TypedSpecDecision {
            result: spec_result(
                input.declared_by,
                ownership.conflicts().to_vec(),
                Vec::new(),
                Vec::new(),
            ),
            outcome: Outcome::Conflicts(ownership),
        });
    }
    let adopted_rule_ids = adopted_rule_ids(&input);
    let rule_relationships = input.rules.clone();
    let spec = input.spec;
    let (mut sources, source_resources) = reconcile::<Source>(
        current.sources,
        &spec,
        scope_id,
        &input.declared_by,
        input.sources,
        &ids,
    )?;
    let (mut requirements, requirement_resources) = reconcile::<Requirement>(
        current.requirements,
        &spec,
        scope_id,
        &input.declared_by,
        input.requirements,
        &ids,
    )?;
    let (mut rules, mut rule_resources) = reconcile::<Rule>(
        current.rules,
        &spec,
        scope_id,
        &input.declared_by,
        input.rules,
        &ids,
    )?;
    let deleted_resources =
        all_resources(&source_resources, &requirement_resources, &rule_resources);
    let cascade = cascade::Cascade::prepare(
        view,
        &deleted_resources,
        &mut sources,
        &mut requirements,
        &mut rules,
    );
    ensure_resolutions_exist(&view.resolutions, &requirements, &rules)?;
    ensure_acyclic(&requirements)?;
    let implementation_reconciliation = reconcile_implementations(
        &current.implementation_bindings,
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
    let mut resources = all_resources(&source_resources, &requirement_resources, &rule_resources);
    let cascade_resources = cascade.report(&mut resources);
    let mut result = spec_result(
        input.declared_by,
        resources,
        cascade_resources,
        implementation_reconciliation.active,
    );
    result.diagnostics = super::super::typed_statement_policy::analyze_typed_statements(
        &result.resources,
        &requirements,
        &rules,
        view.dictionary.as_ref(),
    );
    Ok(TypedSpecDecision {
        result,
        outcome: Outcome::Replace {
            replacement: Box::new(publication::Replacement {
                sources,
                requirements,
                rules,
                implementations: implementation_reconciliation.records,
                cascade,
            }),
            requirement_resources,
            rule_resources,
        },
    })
}

impl CurrentTypedState {
    fn new(owner: &str, view: &TypedSpecView) -> anyhow::Result<Self> {
        let source_addresses = owned_declaration_ids(owner, &view.sources, |record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        })?;
        let requirement_addresses = owned_declaration_ids(owner, &view.requirements, |record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        })?;
        let rule_addresses = owned_declaration_ids(owner, &view.rules, |record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        })?;
        Ok(Self {
            sources: view.sources.clone(),
            requirements: view.requirements.clone(),
            rules: view.rules.clone(),
            implementation_bindings: view.implementation_bindings.clone(),
            source_addresses,
            requirement_addresses,
            rule_addresses,
        })
    }
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
