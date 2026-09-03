use super::{
    AddSourceReferenceInput, CreateEdgeInput, CreateRequirementInput, CreateSourceInput, StateStore,
};
use crate::shards;
use provenance_core::{
    edge_validation::validate_edge_endpoint, validate_optional_commit_pin, Edge, EdgeType,
    NodeType, Requirement, ScopeId, Source, SourceReference, StableId, SUPPORTED_SCHEMA_VERSION,
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
            superseded_by,
            supersedes,
            origin_thread,
            origin_message,
        } = input;
        let commit_pin = validate_optional_commit_pin(commit_pin)?;
        for older in &supersedes {
            self.ensure_node_exists(&scope_id, NodeType::Source, older)?;
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
                superseded_by,
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
            self.ensure_node_exists(&scope_id, NodeType::Domain, domain_id)?;
        }
        for parent in refines.iter().chain(&depends_on).chain(&supersedes) {
            self.ensure_node_exists(&scope_id, NodeType::Requirement, parent)?;
        }
        if let Some(resolution) = &spawned_by {
            self.ensure_node_exists(&scope_id, NodeType::Resolution, resolution)?;
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
        validate_edge_endpoint(
            EdgeType::References,
            NodeType::Source,
            NodeType::Requirement,
        )?;
        anyhow::ensure!(
            self.list_sources(&scope_id)?
                .iter()
                .any(|source| source.id == source_id),
            "source does not exist"
        );
        let source_ref = SourceReference {
            source_id: source_id.clone(),
            clause,
        };
        let requirements_path = shards::requirements_path(&self.layout, &scope_id);
        let requirement = self.mutate_jsonl_records(
            &requirements_path,
            |requirements: &mut Vec<Requirement>| {
                let requirement = requirements
                    .iter_mut()
                    .find(|requirement| requirement.id == requirement_id)
                    .ok_or_else(|| anyhow::anyhow!("requirement does not exist"))?;
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
        self.add_edge(
            scope_id,
            EdgeType::References,
            NodeType::Source,
            source_id,
            NodeType::Requirement,
            requirement_id,
        )?;
        Ok(requirement)
    }

    pub fn create_edge(&self, input: CreateEdgeInput) -> anyhow::Result<Edge> {
        self.create_edge_after_validation(input, || Ok(()))
    }

    pub(super) fn create_edge_after_validation(
        &self,
        input: CreateEdgeInput,
        after_validation: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<Edge> {
        self.with_repository_publication(|| self.write_edge(input, after_validation))
    }

    fn write_edge(
        &self,
        input: CreateEdgeInput,
        after_validation: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<Edge> {
        let CreateEdgeInput {
            scope_id,
            edge_type,
            from_type,
            from_id,
            to_type,
            to_id,
        } = input;
        validate_edge_endpoint(edge_type, from_type, to_type)?;
        self.ensure_edge_endpoint_exists(&scope_id, from_type, &from_id, "from")?;
        self.ensure_edge_endpoint_exists(&scope_id, to_type, &to_id, "to")?;
        after_validation()?;
        self.add_edge(scope_id, edge_type, from_type, from_id, to_type, to_id)
    }

    pub fn delete_edge(&self, scope_id: &ScopeId, id: &StableId) -> anyhow::Result<Edge> {
        let path = shards::edges_path(&self.layout);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Edge>| {
            let index = records
                .iter()
                .position(|record| &record.scope_id == scope_id && &record.id == id)
                .ok_or_else(|| anyhow::anyhow!("edge does not exist"))?;
            Ok(records.remove(index))
        })
    }

    pub(crate) fn add_edge(
        &self,
        scope_id: ScopeId,
        edge_type: EdgeType,
        from_type: NodeType,
        from_id: StableId,
        to_type: NodeType,
        to_id: StableId,
    ) -> anyhow::Result<Edge> {
        validate_edge_endpoint(edge_type, from_type, to_type)?;
        let edge = Edge {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id,
            id: Edge::stable_id(edge_type, from_type, &from_id, to_type, &to_id)?,
            edge_type,
            from_type,
            from_id,
            to_type,
            to_id,
            label: None,
        };
        let path = shards::edges_path(&self.layout);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Edge>| {
            if let Some(existing) = records.iter().find(|record| {
                record.scope_id == edge.scope_id
                    && record.edge_type == edge.edge_type
                    && record.from_type == edge.from_type
                    && record.from_id == edge.from_id
                    && record.to_type == edge.to_type
                    && record.to_id == edge.to_id
            }) {
                return Ok(existing.clone());
            }
            if !records
                .iter()
                .any(|record| record.id == edge.id && record.scope_id == edge.scope_id)
            {
                records.push(edge.clone());
            }
            records.sort_by(|a, b| {
                a.scope_id
                    .as_str()
                    .cmp(b.scope_id.as_str())
                    .then(a.id.as_str().cmp(b.id.as_str()))
            });
            Ok(edge)
        })
    }

    pub(super) fn ensure_edge_endpoint_exists(
        &self,
        scope_id: &ScopeId,
        node_type: NodeType,
        id: &StableId,
        side: &str,
    ) -> anyhow::Result<()> {
        self.ensure_node_exists(scope_id, node_type, id)
            .map_err(|_| anyhow::anyhow!("{side} endpoint does not exist"))
    }
}

/// Lists are sets on write: sorted by id, without duplicates.
pub(super) fn sorted_ids(mut ids: Vec<StableId>) -> Vec<StableId> {
    ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    ids.dedup();
    ids
}
