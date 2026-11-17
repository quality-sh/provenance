//! The reference and scope-ownership checks of a state. They read the state
//! through [`StateStore`] only, so a caller without `host` (the web kernel) runs
//! the same checks as `provenance check`.

use crate::scope_snapshot::{snapshot, ScopeSnapshot};
use crate::state_store::StateStore;
use index::CheckIndex;
use provenance_core::{Manifest, ScopeId, StableId};
use provenance_macros::rule;
use std::collections::BTreeSet;

mod collaboration;
mod core;
mod ideation;
mod index;
mod references;

/// The findings of one check of a state, each list in the order of the
/// manifest scopes and then of the record families.
///
/// A record that another scope owns makes the later checks meaningless, so
/// the review-row and dangling-reference lists stay empty when
/// `scope_ownership` holds a finding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateFindings {
    pub scope_ownership: Vec<String>,
    pub review_rows: Vec<String>,
    pub scope_directories: Vec<String>,
    pub dangling: Vec<String>,
}

impl StateFindings {
    /// The text that `provenance check` reports for these findings, or `None`
    /// for a state with no finding.
    pub fn message(&self) -> Option<String> {
        [
            ("scope ownership finding(s)", &self.scope_ownership),
            ("review row finding(s)", &self.review_rows),
            ("scope directory finding(s)", &self.scope_directories),
            ("dangling reference(s)", &self.dangling),
        ]
        .into_iter()
        .find(|(_, findings)| !findings.is_empty())
        .map(|(title, findings)| format!("{title}:\n- {}", findings.join("\n- ")))
    }
}

/// Checks the references and the scope ownership of the manifest scopes of a state.
#[rule("rule_state_check_without_host")]
pub fn check_state(
    store: &StateStore,
    manifest: &Manifest,
    repository_wide: bool,
) -> anyhow::Result<StateFindings> {
    manifest.ensure_has_scopes()?;
    let manifest_scopes: BTreeSet<_> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.as_str().to_string())
        .collect();

    store.validate_canonical_ids_unique(&manifest.scopes)?;

    let mut findings = StateFindings::default();
    if repository_wide {
        findings.scope_directories = store
            .list_scope_directories()?
            .into_iter()
            .filter(|directory| !manifest_scopes.contains(directory))
            .map(|directory| format!("scope directory {directory} is absent from manifest"))
            .collect();
    }

    let records = manifest
        .scopes
        .iter()
        .map(|scope| ScopeRecords::load(store, &scope.id, &manifest.disposition_actor_ids))
        .collect::<anyhow::Result<Vec<_>>>()?;

    for scope in &records {
        scope.validate_scope_ownership(&manifest_scopes, &mut findings.scope_ownership);
    }
    if !findings.scope_ownership.is_empty() {
        return Ok(findings);
    }
    for scope in &manifest.scopes {
        findings
            .review_rows
            .extend(store.review_row_faults(&scope.id)?);
    }

    let mut index = CheckIndex::default();
    for scope in &records {
        core::Records::load(scope.snapshot.graph_records()).add_to(&mut index);
        collaboration::Records::load(&scope.snapshot).add_to(&mut index);
        ideation::Records::load(&scope.snapshot).add_to(&mut index);
    }
    for scope in &records {
        core::Records::load(scope.snapshot.graph_records()).validate(
            &index,
            &scope.scope_id,
            &mut findings.dangling,
        );
        collaboration::Records::load(&scope.snapshot).validate(
            &index,
            &manifest_scopes,
            &scope.scope_id,
            &mut findings.dangling,
        );
        ideation::Records::load(&scope.snapshot).validate(
            &index,
            &scope.scope_id,
            &mut findings.dangling,
        );
    }
    Ok(findings)
}

struct ScopeRecords {
    scope_id: ScopeId,
    snapshot: ScopeSnapshot,
}

impl ScopeRecords {
    fn load(
        store: &StateStore,
        scope_id: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<Self> {
        store.validate_ideation_scope_with_actor_ids(scope_id, disposition_actor_ids)?;
        store.validate_graph_scope(scope_id)?;
        // The snapshot has no Discussions and no Requirement reviews. Each
        // is read here, so a line of them that does not parse names its
        // file and its line, as a line of the snapshot families does.
        store.list_discussions(scope_id)?;
        store.list_requirement_reviews(scope_id)?;
        Ok(Self {
            scope_id: scope_id.clone(),
            snapshot: snapshot(store, scope_id)?,
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
