use super::{CreateDomainInput, MutationAuth, StateStore};
use crate::shards;
use provenance_core::{Capability, Domain, RbacClaim, SUPPORTED_SCHEMA_VERSION};

impl StateStore {
    pub fn create_domain(
        &self,
        claim: Option<&RbacClaim>,
        input: CreateDomainInput,
    ) -> anyhow::Result<Domain> {
        let CreateDomainInput {
            scope_id,
            id,
            name,
            description,
            color,
        } = input;
        let path = shards::domains_path(&self.layout, &scope_id);
        let auth = MutationAuth::new(claim, Capability::Edit, &scope_id);
        self.mutate_jsonl_records(&path, auth, |records: &mut Vec<Domain>| {
            let domain = Domain {
                schema_version: SUPPORTED_SCHEMA_VERSION,
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
            records.push(domain.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(domain)
        })
    }
}
