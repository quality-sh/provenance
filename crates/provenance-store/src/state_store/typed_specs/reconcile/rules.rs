use provenance_core::{
    DeclarationAddress, Rule, RuleSeverity, RuleStatus, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};

use super::super::rule_addresses::{local_parent, rule_address};
use super::super::{DesiredTypedIds, TypedFieldChange, TypedResourceKind, TypedRuleInput};
use super::changes::{changed, DeclarationRecord};
use super::references;

impl DeclarationRecord for Rule {
    type Declaration = TypedRuleInput;

    const KIND: TypedResourceKind = TypedResourceKind::Rule;

    fn key(declaration: &Self::Declaration) -> &str {
        &declaration.key
    }

    fn address(spec: &str, declaration: &Self::Declaration) -> anyhow::Result<DeclarationAddress> {
        rule_address(spec, declaration)
    }

    fn parent(address: &DeclarationAddress) -> Option<String> {
        local_parent(address)
    }

    fn desired_id<'a>(
        address: &DeclarationAddress,
        _declaration: &Self::Declaration,
        ids: &'a DesiredTypedIds,
    ) -> &'a StableId {
        &ids.rules[address]
    }

    fn desired_contains(ids: &DesiredTypedIds, id: &StableId) -> bool {
        ids.rules.values().any(|desired| desired == id)
    }

    fn desired(
        scope_id: &ScopeId,
        owner: &str,
        address: &DeclarationAddress,
        id: &StableId,
        declaration: &Self::Declaration,
        ids: &DesiredTypedIds,
    ) -> anyhow::Result<Self> {
        let mut rule = Rule {
            created: None,
            updated: None,
            archived_in_commit: None,
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: scope_id.clone(),
            id: id.clone(),
            declared_by: Some(owner.to_string()),
            declaration_address: Some(address.clone()),
            name: declaration.name.clone(),
            description: declaration.description.clone(),
            statement: declaration.statement.clone(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            requirement_ids: Vec::new(),
            resolution_ids: Vec::new(),
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        };
        references::apply_rule(&mut rule, declaration, &ids.requirements)?;
        Ok(rule)
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
        if desired.name.is_some() {
            reconciled.name = desired.name;
        }
        if desired.description.is_some() {
            reconciled.description = desired.description;
        }
        references::apply_rule(&mut reconciled, declaration, &ids.requirements)?;
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
        changed(&mut changes, "name", &self.name, &after.name);
        changed(
            &mut changes,
            "description",
            &self.description,
            &after.description,
        );
        changed(
            &mut changes,
            "requirement_ids",
            &self.requirement_ids,
            &after.requirement_ids,
        );
        changed(
            &mut changes,
            "resolution_ids",
            &self.resolution_ids,
            &after.resolution_ids,
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
