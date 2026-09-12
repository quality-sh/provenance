use crate::output;
use crate::store::Store;
use camino::Utf8PathBuf;
use provenance_core::{ScopeId, StableId};
use provenance_store::cache;

pub(super) fn handle(rule_id: String, repo: Utf8PathBuf, scope: String) -> anyhow::Result<()> {
    let store = Store::open(repo);
    let trace = cache::trace_rule(
        store.layout(),
        &ScopeId::new(scope)?,
        &StableId::new(rule_id)?,
    )?;
    output::print_json(&trace)?;
    Ok(())
}
