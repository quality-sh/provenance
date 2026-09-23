use super::{CreateRequirementInput, CreateSourceInput, StateStore};
use crate::shards;
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{
    validate_optional_commit_pin, NodeType, Requirement, Source, StableId, SUPPORTED_SCHEMA_VERSION,
};

impl StateStore {
    pub fn create_source(&self, input: CreateSourceInput) -> anyhow::Result<Source> {
        self.with_repository_publication(|| {
            let CreateSourceInput {
                scope_id,
                id,
                name,
                source_type,
                url,
                reference,
                commit_pin,
                effective_date,
                review_date,
                supersedes,
                origin_thread,
                origin_message,
            } = input;
            self.ensure_canonical_id_available(&scope_id, &id)?;
            let commit_pin = validate_optional_commit_pin(commit_pin)
                .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidCommitPin, error))?;
            for older in &supersedes {
                self.ensure_node_exists(&scope_id, NodeType::Source, older, "--supersedes")?;
            }
            crate::test_probes::at("source_supersedes_validated")?;
            let supersedes = sorted_ids(supersedes);
            let path = shards::sources_path(&self.layout, &scope_id);
            self.mutate_graph_record(&path, |records: &mut Vec<Source>| {
                let source = Source {
                    created: None,
                    updated: None,
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: scope_id.clone(),
                    id,
                    declared_by: None,
                    declaration_address: None,

                    name,
                    source_type,
                    url,
                    reference,
                    commit_pin,
                    effective_date,
                    review_date,
                    supersedes,
                    origin_thread,
                    origin_message,
                };
                crate::write_error::ensure!(
                    AlreadyExists,
                    !records.iter().any(|record| record.id == source.id),
                    "source already exists"
                );
                records.push(source.clone());
                records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
                Ok(source)
            })
        })
    }

    /// Creates a Requirement only within the guarded creation path.
    pub(crate) fn write_requirement(
        &self,
        input: CreateRequirementInput,
    ) -> anyhow::Result<Requirement> {
        let CreateRequirementInput {
            scope_id,
            id,
            statement,
            description,
            status,
            domain_id,
            refines,
            depends_on,
            supersedes,
            spawned_by,
            origin_thread,
            origin_message,
        } = input;
        self.ensure_canonical_id_available(&scope_id, &id)?;
        super::statement_policy::ensure_statement_is_writable(&self.layout, &statement)?;
        if let Some(domain_id) = &domain_id {
            self.ensure_node_exists(&scope_id, NodeType::Domain, domain_id, "--domain-id")?;
        }
        if let Some(parent) = &refines {
            self.ensure_node_exists(&scope_id, NodeType::Requirement, parent, "--refines")?;
        }
        for dependency in &depends_on {
            self.ensure_node_exists(&scope_id, NodeType::Requirement, dependency, "--depends-on")?;
        }
        for older in &supersedes {
            self.ensure_node_exists(&scope_id, NodeType::Requirement, older, "--supersedes")?;
        }
        if let Some(resolution) = &spawned_by {
            self.ensure_node_exists(&scope_id, NodeType::Resolution, resolution, "--spawned-by")?;
        }
        let (depends_on, supersedes) = (sorted_ids(depends_on), sorted_ids(supersedes));
        let path = shards::requirements_path(&self.layout, &scope_id);
        self.mutate_graph_record(&path, |records: &mut Vec<Requirement>| {
            let requirement = Requirement {
                created: None,
                updated: None,
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                declared_by: None,
                declaration_address: None,

                statement,
                description,
                fog: None,
                status,
                domain_id,
                source_refs: Vec::new(),
                refines,
                depends_on,
                supersedes,
                spawned_by,
                origin_thread,
                origin_message,
            };
            crate::write_error::ensure!(
                AlreadyExists,
                !records.iter().any(|record| record.id == requirement.id),
                "requirement already exists"
            );
            records.push(requirement.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(requirement)
        })
    }
}

/// Lists are sets on write: sorted by id, without duplicates.
pub(super) fn sorted_ids(mut ids: Vec<StableId>) -> Vec<StableId> {
    ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    ids.dedup();
    ids
}
