use provenance_core::authoring::addresses;
use provenance_core::{
    DeclarationAddress, ScopeId, Source, SourceType, StableId, SUPPORTED_SCHEMA_VERSION,
};

use super::super::DesiredTypedIds;
use super::changes::{changed, DeclarationRecord};
use super::references;
use crate::state_store::{TypedFieldChange, TypedResourceKind, TypedSourceInput};

impl DeclarationRecord for Source {
    type Declaration = TypedSourceInput;

    const KIND: TypedResourceKind = TypedResourceKind::Source;

    fn key(declaration: &Self::Declaration) -> &str {
        &declaration.key
    }

    fn address(spec: &str, declaration: &Self::Declaration) -> anyhow::Result<DeclarationAddress> {
        addresses::source_address(spec, &declaration.key)
    }

    fn parent(_address: &DeclarationAddress) -> Option<String> {
        None
    }

    fn desired_id<'a>(
        _address: &DeclarationAddress,
        declaration: &Self::Declaration,
        ids: &'a DesiredTypedIds,
    ) -> &'a StableId {
        &ids.sources[&declaration.key]
    }

    fn desired_contains(ids: &DesiredTypedIds, id: &StableId) -> bool {
        ids.sources.values().any(|desired| desired == id)
    }

    fn desired(
        scope_id: &ScopeId,
        owner: &str,
        address: &DeclarationAddress,
        id: &StableId,
        declaration: &Self::Declaration,
        ids: &DesiredTypedIds,
    ) -> anyhow::Result<Self> {
        let mut source = Source {
            created: None,
            updated: None,
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope_id.clone(),
            id: id.clone(),
            declared_by: Some(owner.to_string()),
            declaration_address: Some(address.clone()),
            name: declaration.name.clone(),
            source_type: source_type(&declaration.kind)?,
            url: declaration.url.clone(),
            reference: declaration.reference.clone(),
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        };
        references::apply_source(&mut source, declaration, &ids.sources);
        Ok(source)
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
        reconciled.name = desired.name;
        reconciled.source_type = desired.source_type;
        if desired.url.is_some() {
            reconciled.url = desired.url;
        }
        if desired.reference.is_some() {
            reconciled.reference = desired.reference;
        }
        references::apply_source(&mut reconciled, declaration, &ids.sources);
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
        changed(&mut changes, "name", &self.name, &after.name);
        changed(&mut changes, "kind", &self.source_type, &after.source_type);
        changed(&mut changes, "url", &self.url, &after.url);
        changed(&mut changes, "reference", &self.reference, &after.reference);
        changed(
            &mut changes,
            "supersedes",
            &self.supersedes,
            &after.supersedes,
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

fn source_type(kind: &str) -> anyhow::Result<SourceType> {
    SourceType::parse(kind).or_else(|_| match kind.to_ascii_lowercase().as_str() {
        "linear" | "github" | "jira" => Ok(SourceType::ExternalIntegration),
        _ => Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::InvalidDeclaration,
            anyhow::anyhow!("source kind `{kind}` is not supported"),
        )),
    })
}
