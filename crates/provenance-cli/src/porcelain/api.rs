use super::OutputFormat;
use crate::repo_context::RepoContext;
use provenance_porcelain::api::{render_discovery_readable, ApiOutcome, ApiRequest};

/// Run one api action through the shared porcelain port, in process, with
/// the configured repository, scope, and local credential context.
#[provenance_macros::rule("rule_porcelain_api_uses_context")]
pub async fn dispatch_api(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    request: ApiRequest,
) -> anyhow::Result<()> {
    let host = RepoContext::new(repo, scope).local_host()?;
    let service = host.porcelain().api();
    match service.execute_api(request).await {
        Ok(ApiOutcome::Catalog(catalog)) => {
            if format == Some(OutputFormat::Json) {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            } else {
                println!("{}", render_discovery_readable(&catalog));
                println!("Use --format json for the full request and response schemas.");
            }
        }
        Ok(ApiOutcome::Invoked(value)) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(error) => anyhow::bail!("{}", serde_json::to_string(&error.failure)?),
    }
    Ok(())
}
