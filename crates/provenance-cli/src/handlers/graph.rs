use crate::output;
use crate::store::Store;
use camino::Utf8PathBuf;
use provenance_core::{ScopeId, StableId};
use provenance_store::cache;

pub(super) fn handle(
    requirement_id: String,
    repo: Utf8PathBuf,
    scope: String,
) -> anyhow::Result<()> {
    let store = Store::open(repo);
    let graph = cache::get_requirement_graph(
        store.layout(),
        &ScopeId::new(scope)?,
        &StableId::new(requirement_id)?,
    )?;
    output::print_json(&graph)?;
    Ok(())
}
