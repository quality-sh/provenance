use provenance_core::authoring::addresses;
use provenance_core::{
    DeclarationAddress, Requirement, RequirementStatus, ScopeId, SourceReference, StableId,
    SUPPORTED_SCHEMA_VERSION,
};

use super::super::DesiredTypedIds;
use super::changes::{changed, DeclarationRecord};
use super::references;
use crate::state_store::{TypedFieldChange, TypedRequirementInput, TypedResourceKind};

impl DeclarationRecord for Requirement {
    type Declaration = TypedRequirementInput;

    const KIND: TypedResourceKind = TypedResourceKind::Requirement;

    fn key(declaration: &Self::Declaration) -> &str {
        &declaration.key
    }

    fn address(spec: &str, declaration: &Self::Declaration) -> anyhow::Result<DeclarationAddress> {
        addresses::requirement_address(spec, &declaration.key)
    }

    fn parent(_address: &DeclarationAddress) -> Option<String> {
        None
    }

    fn desired_id<'a>(
        _address: &DeclarationAddress,
        declaration: &Self::Declaration,
        ids: &'a DesiredTypedIds,
    ) -> &'a StableId {
        &ids.requirements[&declaration.key]
    }

    fn desired_contains(ids: &DesiredTypedIds, id: &StableId) -> bool {
        ids.requirements.values().any(|desired| desired == id)
    }

    fn desired(
        scope_id: &ScopeId,
        owner: &str,
        address: &DeclarationAddress,
        id: &StableId,
        declaration: &Self::Declaration,
        ids: &DesiredTypedIds,
    ) -> anyhow::Result<Self> {
        let source_refs = declaration
            .sources
            .iter()
            .map(|key| SourceReference {
                source_id: ids.sources[key].clone(),
                clause: None,
            })
            .collect();
        let mut requirement = Requirement {
            created: None,
            updated: None,
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope_id.clone(),
            id: id.clone(),
            declared_by: Some(owner.to_string()),
            declaration_address: Some(address.clone()),
            statement: declaration.statement.clone(),
            description: declaration.description.clone(),
            fog: None,
            status: RequirementStatus::Active,
            domain_id: None,
            source_refs,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        };
        references::apply_requirement(&mut requirement, declaration, &ids.requirements)?;
        Ok(requirement)
    }

    fn reconciled(
        &self,
        desired: Self,
        declaration: &Self::Declaration,
        ids: &DesiredTypedIds,
    ) -> anyhow::Result<Self> {
        let mut reconciled = self.clone();
        reconciled.declared_by = desired.declared_by;
        reconciled.declaration_address = desired.declaration_address;
        reconciled.statement = desired.statement;
        if desired.description.is_some() {
            reconciled.description = desired.description;
        }
        for source in desired.source_refs {
            if !reconciled
                .source_refs
                .iter()
                .any(|existing| existing.source_id == source.source_id)
            {
                reconciled.source_refs.push(source);
            }
        }
        reconciled.source_refs.sort_by(|left, right| {
            left.source_id
                .as_str()
                .cmp(right.source_id.as_str())
                .then(left.clause.cmp(&right.clause))
        });
        references::apply_requirement(&mut reconciled, declaration, &ids.requirements)?;
        Ok(reconciled)
    }

    fn changes(&self, after: &Self) -> Vec<TypedFieldChange> {
        let mut changes = Vec::new();
        changed(
            &mut changes,
            "declared_by",
            &self.declared_by,
            &after.declared_by,
        );
        changed(
            &mut changes,
            "address",
            &self.declaration_address,
            &after.declaration_address,
        );
        changed(&mut changes, "statement", &self.statement, &after.statement);
        changed(
            &mut changes,
            "description",
            &self.description,
            &after.description,
        );
        changed(
            &mut changes,
            "sources",
            &self.source_refs,
            &after.source_refs,
        );
        changed(&mut changes, "refines", &self.refines, &after.refines);
        changed(
            &mut changes,
            "depends_on",
            &self.depends_on,
            &after.depends_on,
        );
        changed(
            &mut changes,
            "supersedes",
            &self.supersedes,
            &after.supersedes,
        );
        changed(
            &mut changes,
            "spawned_by",
            &self.spawned_by,
            &after.spawned_by,
        );
        changes
    }

    fn stable_id(&self) -> &StableId {
        &self.id
    }

    fn declared_by(&self) -> Option<&str> {
        self.declared_by.as_deref()
    }

    fn declaration_address(&self) -> Option<&DeclarationAddress> {
        self.declaration_address.as_ref()
    }

    fn copy_declaration_identity(&mut self, desired: &Self) {
        self.declared_by.clone_from(&desired.declared_by);
        self.declaration_address
            .clone_from(&desired.declaration_address);
    }
}
