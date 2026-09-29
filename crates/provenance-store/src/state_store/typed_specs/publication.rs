use provenance_core::{ImplementationBinding, NodeType, Requirement, Rule, ScopeId, Source};

use super::{cascade::Cascade, replace_records};
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
        let replacements = self
            .sources
            .iter()
            .map(|record| (Some(NodeType::Source), &record.id))
            .chain(
                self.requirements
                    .iter()
                    .map(|record| (Some(NodeType::Requirement), &record.id)),
            )
            .chain(
                self.rules
                    .iter()
                    .map(|record| (Some(NodeType::Rule), &record.id)),
            );
        store.ensure_canonical_replacement_ids_unique(
            scope,
            replacements,
            &[NodeType::Source, NodeType::Requirement, NodeType::Rule],
        )?;
        super::super::typed_statement_policy::ensure_typed_spec_is_writable(result)?;
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
        let mut desired = self
            .sources
            .iter()
            .cloned()
            .map(Into::into)
            .chain(self.requirements.iter().cloned().map(Into::into))
            .chain(self.rules.iter().cloned().map(Into::into))
            .collect::<Vec<_>>();
        desired.extend(store.list_domains(scope)?.into_iter().map(Into::into));
        self.cascade.extend_review_records(&mut desired);
        store.publish_typed_spec(scope, &result.declared_by, &desired, |store| {
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
        })
        .map_err(publication_started)
    }
}
