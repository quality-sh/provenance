use super::{local_host, OutputFormat};
use provenance_porcelain::api::{ApiOutcome, ApiRequest};

/// Run one api action through the shared porcelain port, in process.
pub async fn dispatch_api(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    request: ApiRequest,
) -> anyhow::Result<()> {
    let _ = (repo, scope, format, request);
    todo!("cli api dispatch")
}
