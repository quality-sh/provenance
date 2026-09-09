//! Native framing for the two complete verification lists.
use crate::output::{self, OutputFormat};
use camino::Utf8PathBuf;
use provenance_core::{protocol::repository::VerificationListRequest, ScopeId, StableId};
use provenance_store::operations::{self, catalog};

pub(super) async fn print<O>(
    repo: Option<Utf8PathBuf>,
    scope: String,
    rule: Option<String>,
    format: OutputFormat,
) -> anyhow::Result<()>
where
    O: catalog::Operation<Request = VerificationListRequest>,
    O::Failure: Sync,
{
    let root = operations::discover_repository(repo)?;
    let rule = rule.map(StableId::new).transpose()?;
    let context = catalog::PreparedContext::for_scope(catalog::PreparedScope {
        root,
        scope: ScopeId::new(scope)?,
        requested_target: String::new(),
    });
    let result = catalog::invoke_typed::<O>(context, VerificationListRequest { rule }).await?;
    output::print(format, &result)
}
