use crate::output;
use camino::Utf8PathBuf;
use provenance_store::{cache, layout::ProvenanceLayout};

pub(super) async fn handle(repo: Utf8PathBuf) -> anyhow::Result<()> {
    let report = cache::materialize_state(&ProvenanceLayout::new(repo)).await?;
    output::print_json(&report)?;
    Ok(())
}
