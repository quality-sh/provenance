use provenance_core::{ImplementationBinding, Requirement, Rule, ScopeId, Source};

use super::{cascade::Cascade, replace_records};
use crate::review::guard::protect_requirements;
use crate::state_store::{ReconciledResource, StateStore, TypedSpecResult};
use crate::{shards, write_error::publication_started};

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
        (|| -> anyhow::Result<()> {
            store
                .replace_graph_records(&shards::sources_path(&store.layout, scope), self.sources)?;
            crate::test_probes::at("typed_spec_sources_published")?;
            store.replace_graph_records(
                &shards::requirements_path(&store.layout, scope),
                self.requirements,
            )?;
            store.replace_graph_records(&shards::rules_path(&store.layout, scope), self.rules)?;
            replace_records(
                store,
                &shards::implementation_bindings_path(&store.layout, scope),
                self.implementations,
            )?;
            self.cascade.publish(store, scope)?;
            store.raise_requirement_reviews(scope, requirement_resources, rule_resources)
        })()
        .map_err(publication_started)
    }
}
