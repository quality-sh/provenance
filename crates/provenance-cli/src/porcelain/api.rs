use super::{local_host, OutputFormat};
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
    let host = local_host(repo, scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostApiPort::new(host),
    );
    match service.execute_api(request).await {
        Ok(ApiOutcome::Catalog(catalog)) => {
            if format == Some(OutputFormat::Json) {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            } else {
                println!("{}", render_discovery_readable(&catalog));
            }
        }
        Ok(ApiOutcome::Invoked(value)) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(error) => anyhow::bail!("{}", serde_json::to_string(&error.failure)?),
    }
    Ok(())
}
