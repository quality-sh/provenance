use crate::output;
use camino::Utf8PathBuf;
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_store::{cache, layout::ProvenanceLayout};

pub(super) fn handle(
    id: String,
    repo: Utf8PathBuf,
    scope: String,
    node_type: &str,
    max_hops: u32,
    follow_indirect: bool,
) -> anyhow::Result<()> {
    let view = cache::analyze_impact(
        &ProvenanceLayout::new(repo),
        &ScopeId::new(scope)?,
        NodeType::parse(node_type)?,
        &StableId::new(id)?,
        cache::ImpactOptions {
            max_hops,
            follow_indirect,
        },
    )?;
    output::print_json(&view)?;
    Ok(())
}
