use super::read_budget::ensure_within_read_budget;
use super::{CreateDomainInput, StateStore};
use crate::shards;
use provenance_core::{review::REVIEW_SCHEMA_VERSION, Domain};

impl StateStore {
    pub fn create_domain(&self, input: CreateDomainInput) -> anyhow::Result<Domain> {
        let path = shards::domains_path(&self.layout, &input.scope_id);
        let id = input.id.clone();
        self.create_native_record(&path, &id, |store| store.write_domain(input))
    }

    fn write_domain(&self, input: CreateDomainInput) -> anyhow::Result<Domain> {
        let CreateDomainInput {
            scope_id,
            id,
            name,
            description,
            color,
        } = input;
        self.ensure_canonical_id_available(&scope_id, &id)?;
        let path = shards::domains_path(&self.layout, &scope_id);
        self.mutate_graph_record(&path, |records: &mut Vec<Domain>| {
            let domain = Domain {
                schema_version: REVIEW_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                name,
                description,
                color,
            };
            anyhow::ensure!(
                !records.iter().any(|record| record.id == domain.id),
                "domain already exists"
            );
            anyhow::ensure!(
                !records.iter().any(|record| record.name == domain.name),
                "domain name already exists"
            );
            ensure_within_read_budget(&domain)?;
            records.push(domain.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(domain)
        })
    }
}
