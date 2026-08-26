use super::index::CheckIndex;
use provenance_core::{Scope, ScopeId, StableId};
use provenance_store::state_store::StateStore;
use std::collections::BTreeSet;

mod collaboration;
mod core;
mod ideation;

struct ScopeRecords {
    scope_id: ScopeId,
    core: core::Records,
    collaboration: collaboration::Records,
    ideation: ideation::Records,
}

impl ScopeRecords {
    fn load(
        store: &StateStore,
        scope_id: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            scope_id: scope_id.clone(),
            core: core::Records::load(store, scope_id)?,
            collaboration: collaboration::Records::load(store, scope_id)?,
            ideation: ideation::Records::load(store, scope_id, disposition_actor_ids)?,
        })
    }

    fn validate_scope_ownership(
        &self,
        manifest_scopes: &BTreeSet<String>,
        findings: &mut Vec<String>,
    ) {
        self.core.validate_scope_ownership(&self.scope_id, findings);
        self.collaboration
            .validate_scope_ownership(manifest_scopes, &self.scope_id, findings);
        self.ideation
            .validate_scope_ownership(&self.scope_id, findings);
    }
}

fn check_scope_ownership(
    loaded_scope_id: &ScopeId,
    embedded_scope_id: &ScopeId,
    record_type: &str,
    record_id: &StableId,
    findings: &mut Vec<String>,
) {
    if loaded_scope_id != embedded_scope_id {
        findings.push(format!(
            "{record_type} {} loaded from scope {} has embedded scope_id {}",
            record_id.as_str(),
            loaded_scope_id.as_str(),
            embedded_scope_id.as_str()
        ));
    }
}

pub(super) fn validate(
    store: &StateStore,
    scopes: &[Scope],
    disposition_actor_ids: &[String],
    manifest_scopes: &BTreeSet<String>,
    index: &mut CheckIndex,
    dangling: &mut Vec<String>,
) -> anyhow::Result<()> {
    let records = scopes
        .iter()
        .map(|scope| ScopeRecords::load(store, &scope.id, disposition_actor_ids))
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut ownership_findings = Vec::new();
    for scope in &records {
        scope.validate_scope_ownership(manifest_scopes, &mut ownership_findings);
    }
    anyhow::ensure!(
        ownership_findings.is_empty(),
        "scope ownership finding(s):\n- {}",
        ownership_findings.join("\n- ")
    );

    for scope in &records {
        scope.core.add_to(index);
        scope.collaboration.add_to(index);
        scope.ideation.add_to(index);
    }
    for scope in &records {
        scope.core.validate(index, &scope.scope_id, dangling);
        scope
            .collaboration
            .validate(index, manifest_scopes, &scope.scope_id, dangling);
        scope.ideation.validate(index, &scope.scope_id, dangling);
    }

    Ok(())
}
