use provenance_core::{ImplementationBinding, Requirement, Rule, ScopeId, Source};

use super::{cascade::Cascade, replace_records};
use crate::review::guard::protect_requirements;
use crate::state_store::{ReconciledResource, StateStore, TypedSpecResult};
use crate::{publication::with_staged_state, shards};

pub(super) struct Replacement {
    pub sources: Vec<Source>,
    pub requirements: Vec<Requirement>,
    pub rules: Vec<Rule>,
    pub implementations: Vec<ImplementationBinding>,
    pub cascade: Cascade,
}

impl Replacement {
    pub(super) fn publish(
        self,
        store: &StateStore,
        scope: &ScopeId,
        result: &TypedSpecResult,
        requirement_resources: &[ReconciledResource],
        rule_resources: &[ReconciledResource],
    ) -> anyhow::Result<()> {
        super::super::typed_statement_policy::ensure_typed_spec_is_writable(result)?;
        protect_requirements(&store.layout, scope, &self.requirements)?;
        for rule in &self.rules {
            rule.validate_archive()?;
        }
        self.cascade.ensure_dispositions_survive(
            store,
            scope,
            &self.sources,
            &self.requirements,
            &self.rules,
        )?;
        with_staged_state(&store.layout, false, |layout| {
            let staged = StateStore::new(layout.clone());
            staged.replace_graph_records(&shards::sources_path(layout, scope), self.sources)?;
            crate::test_probes::at("typed_spec_sources_published")?;
            staged.replace_graph_records(
                &shards::requirements_path(layout, scope),
                self.requirements,
            )?;
            staged.replace_graph_records(&shards::rules_path(layout, scope), self.rules)?;
            replace_records(
                &staged,
                &shards::implementation_bindings_path(layout, scope),
                self.implementations,
            )?;
            self.cascade.publish(&staged, scope)?;
            staged.raise_requirement_reviews(scope, requirement_resources, rule_resources)
        })
    }
}
