//! Reconciles one typed declaration document with the canonical records,
//! one file per record kind and one for the reference fields they share.

mod changes;
mod references;
mod requirements;
mod rules;
mod sources;

pub(super) use references::{ensure_acyclic, ensure_resolutions_exist};
pub(super) use requirements::{
    desired_requirement, reconcile_requirements, reconciled_requirement, requirement_changes,
};
pub(super) use rules::{desired_rule, reconcile_rules, reconciled_rule, rule_changes};
pub(super) use sources::{desired_source, reconcile_sources, reconciled_source, source_changes};
