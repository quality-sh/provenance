use crate::layout::ProvenanceLayout;

pub(super) fn recover_pending_source_edit(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let marker = layout.source_edit_marker_path();
    if !marker.exists() {
        return Ok(());
    }
    let _: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::bail!("source-edit publication marker cannot be recovered")
}
