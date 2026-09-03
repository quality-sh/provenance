use super::{AddSourceReferenceInput, CreateRequirementInput, CreateSourceInput, StateStore};
use crate::shards;
use provenance_core::{
    validate_optional_commit_pin, NodeType, Requirement, ScopeId, Source, SourceReference,
    StableId, SUPPORTED_SCHEMA_VERSION,
};

impl StateStore {
    pub fn create_source(&self, input: CreateSourceInput) -> anyhow::Result<Source> {
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
        let commit_pin = validate_optional_commit_pin(commit_pin)?;
        for older in &supersedes {
            self.ensure_node_exists(&scope_id, NodeType::Source, older, "--supersedes")?;
        }
        let supersedes = sorted_ids(supersedes);
        let path = shards::sources_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Source>| {
            let source = Source {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                declared_by: None,
                declaration_address: None,
                retired: false,
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
            anyhow::ensure!(
                !records.iter().any(|record| record.id == source.id),
                "source already exists"
            );
            records.push(source.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(source)
        })
    }

    pub fn create_requirement(&self, input: CreateRequirementInput) -> anyhow::Result<Requirement> {
        self.with_repository_publication(|| self.write_requirement(input))
    }

    fn write_requirement(&self, input: CreateRequirementInput) -> anyhow::Result<Requirement> {
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
        self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
            let requirement = Requirement {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                declared_by: None,
                declaration_address: None,
                retired: false,
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
            anyhow::ensure!(
                !records.iter().any(|record| record.id == requirement.id),
                "requirement already exists"
            );
            records.push(requirement.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(requirement)
        })
    }

    /// Set (`Some`) or clear (`None`) the deliberately unstructured fog text
    /// on a requirement.
    pub fn set_requirement_fog(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        fog: Option<String>,
    ) -> anyhow::Result<Requirement> {
        if let Some(fog) = &fog {
            anyhow::ensure!(!fog.trim().is_empty(), "fog text must not be empty");
        }
        let path = shards::requirements_path(&self.layout, scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
            let requirement = records
                .iter_mut()
                .find(|requirement| &requirement.id == id)
                .ok_or_else(|| anyhow::anyhow!("requirement does not exist"))?;
            requirement.fog = fog;
            Ok(requirement.clone())
        })
    }

    pub fn add_source_reference(
        &self,
        input: AddSourceReferenceInput,
    ) -> anyhow::Result<Requirement> {
        self.with_repository_publication(|| self.write_source_reference(input))
    }

    fn write_source_reference(
        &self,
        input: AddSourceReferenceInput,
    ) -> anyhow::Result<Requirement> {
        let AddSourceReferenceInput {
            scope_id,
            source_id,
            requirement_id,
            clause,
        } = input;
        anyhow::ensure!(
            self.list_sources(&scope_id)?
                .iter()
                .any(|source| source.id == source_id),
            "source {} does not exist (--target-id)",
            source_id.as_str()
        );
        let source_ref = SourceReference { source_id, clause };
        let requirements_path = shards::requirements_path(&self.layout, &scope_id);
        let requirement = self.mutate_jsonl_records(
            &requirements_path,
            |requirements: &mut Vec<Requirement>| {
                let requirement = requirements
                    .iter_mut()
                    .find(|requirement| requirement.id == requirement_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "requirement {} does not exist (--requirement-id)",
                            requirement_id.as_str()
                        )
                    })?;
                if !requirement
                    .source_refs
                    .iter()
                    .any(|existing| existing == &source_ref)
                {
                    requirement.source_refs.push(source_ref);
                    requirement.source_refs.sort_by(|a, b| {
                        a.source_id
                            .as_str()
                            .cmp(b.source_id.as_str())
                            .then(a.clause.cmp(&b.clause))
                    });
                }
                Ok(requirement.clone())
            },
        )?;
        Ok(requirement)
    }
}

/// Lists are sets on write: sorted by id, without duplicates.
pub(super) fn sorted_ids(mut ids: Vec<StableId>) -> Vec<StableId> {
    ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    ids.dedup();
    ids
}
