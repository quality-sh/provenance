use crate::output;
use crate::store::Store;
use camino::Utf8PathBuf;
use provenance_store::cache;

pub(super) async fn handle(repo: Utf8PathBuf) -> anyhow::Result<()> {
    let store = Store::open(repo);
    let report = cache::materialize_state(store.layout()).await?;
    output::print_json(&report)?;
    Ok(())
}
