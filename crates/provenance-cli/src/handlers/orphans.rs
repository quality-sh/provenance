use crate::output;
use crate::store::Store;
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::cache;

pub(super) fn handle(repo: Utf8PathBuf, scope: String) -> anyhow::Result<()> {
    let store = Store::open(repo);
    let orphans = cache::orphan_rules(store.layout(), &ScopeId::new(scope)?)?;
    output::print_json(&orphans)?;
    Ok(())
}
