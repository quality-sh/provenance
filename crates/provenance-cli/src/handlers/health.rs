use crate::output;
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::{cache, layout::ProvenanceLayout};

pub(super) fn handle(repo: Utf8PathBuf, scope: String) -> anyhow::Result<()> {
    let health = cache::coverage_health(&ProvenanceLayout::new(repo), &ScopeId::new(scope)?)?;
    output::print_json(&health)?;
    Ok(())
}
