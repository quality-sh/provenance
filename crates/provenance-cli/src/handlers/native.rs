//! The one native seam between CLI handlers and catalog operations.
//!
//! A native caller prepares a scope context once and runs the operation it
//! already names. Host framing — protocol version checks, JSON envelopes,
//! target resolution — stays outside this seam.

use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::operations::catalog::{
    invoke_typed, Operation, PreparedContext, PreparedScope,
};

/// How a native caller names the repository target it addressed.
///
/// Hosts resolve this name against their own repository list; nothing on the
/// native paths reads the value. `native` says the caller addressed the
/// repository by path, not by a host-assigned name.
const REQUESTED_TARGET: &str = "native";

/// Prepares the scope context and runs one catalog operation.
pub(super) async fn invoke_native<O>(
    root: Utf8PathBuf,
    scope: ScopeId,
    request: O::Request,
) -> anyhow::Result<O::Success>
where
    O: Operation,
    O::Failure: std::error::Error + Send + Sync + 'static,
{
    let context = PreparedContext::for_scope(PreparedScope {
        root,
        scope,
        requested_target: REQUESTED_TARGET.into(),
    });
    Ok(invoke_typed::<O>(context, request).await?)
}
