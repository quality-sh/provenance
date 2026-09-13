//! Native framing for the two complete verification lists.
use crate::handlers::native::invoke_native;
use crate::output;
use camino::Utf8PathBuf;
use provenance_core::{protocol::repository::VerificationListRequest, ScopeId, StableId};
use provenance_store::operations::{self, catalog};

pub(super) async fn print<O>(
    repo: Option<Utf8PathBuf>,
    scope: String,
    rule: Option<String>,
) -> anyhow::Result<()>
where
    O: catalog::Operation<Request = VerificationListRequest>,
    O::Failure: Sync,
{
    let root = operations::discover_repository(repo)?;
    let rule = rule.map(StableId::new).transpose()?;
    let result =
        invoke_native::<O>(root, ScopeId::new(scope)?, VerificationListRequest { rule }).await?;
    output::print_json(&result)
}
