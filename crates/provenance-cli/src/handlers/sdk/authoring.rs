//! Native authoring keeps stdin and output framing outside the operation catalog.
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::operations::{self, catalog};

pub(super) async fn invoke<O>(
    root: Utf8PathBuf,
    scope: String,
    input: O::Request,
) -> anyhow::Result<O::Success>
where
    O: catalog::Operation,
    O::Failure: Sync,
{
    let context = catalog::PreparedContext::for_scope(catalog::PreparedScope {
        root,
        scope: ScopeId::new(scope)?,
        requested_target: String::new(),
    });
    Ok(operations::catalog::invoke_typed::<O>(context, input).await?)
}
