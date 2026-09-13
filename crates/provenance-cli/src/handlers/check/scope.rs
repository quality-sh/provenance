use super::index::CheckIndex;
use crate::store::{ScopeSnapshot, Store};
use provenance_core::{Scope, ScopeId, StableId};
use std::collections::BTreeSet;

mod collaboration;
mod core;
mod ideation;

struct ScopeRecords {
    scope_id: ScopeId,
    snapshot: ScopeSnapshot,
}

impl ScopeRecords {
    fn load(
        store: &Store,
        scope_id: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<Self> {
        store.validate_ideation_scope_with_actor_ids(scope_id, disposition_actor_ids)?;
        store.validate_graph_scope(scope_id)?;
        Ok(Self {
            scope_id: scope_id.clone(),
            snapshot: store.snapshot(scope_id)?,
        })
    }

    fn validate_scope_ownership(
        &self,
        manifest_scopes: &BTreeSet<String>,
        findings: &mut Vec<String>,
    ) {
        core::Records::load(self.snapshot.graph_records())
            .validate_scope_ownership(&self.scope_id, findings);
        collaboration::Records::load(&self.snapshot).validate_scope_ownership(
            manifest_scopes,
            &self.scope_id,
            findings,
        );
        ideation::Records::load(&self.snapshot).validate_scope_ownership(&self.scope_id, findings);
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
    store: &Store,
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
        core::Records::load(scope.snapshot.graph_records()).add_to(index);
        collaboration::Records::load(&scope.snapshot).add_to(index);
        ideation::Records::load(&scope.snapshot).add_to(index);
    }
    for scope in &records {
        core::Records::load(scope.snapshot.graph_records()).validate(
            index,
            &scope.scope_id,
            dangling,
        );
        collaboration::Records::load(&scope.snapshot).validate(
            index,
            manifest_scopes,
            &scope.scope_id,
            dangling,
        );
        ideation::Records::load(&scope.snapshot).validate(index, &scope.scope_id, dangling);
    }

    Ok(())
}
