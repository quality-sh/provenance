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
    filter: Option<&str>,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    let host = RepoContext::new(repo, scope).local_host()?;
    let service = host.porcelain().api();
    match service.execute_api(request).await {
        Ok(ApiOutcome::Catalog(catalog)) => {
            if format == Some(OutputFormat::Json) {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            } else {
                let total = catalog.routes.len();
                let needle = filter.map(str::to_ascii_lowercase);
                let mut routes = catalog
                    .routes
                    .into_iter()
                    .filter(|route| {
                        needle.as_ref().is_none_or(|needle| {
                            format!(
                                "{} {} {}",
                                route.method.as_str(),
                                route.path,
                                route.description
                            )
                            .to_ascii_lowercase()
                            .contains(needle)
                        })
                    })
                    .collect::<Vec<_>>();
                let matching = routes.len();
                routes.truncate(limit.unwrap_or(50));
                println!(
                    "{}",
                    render_discovery_readable(&provenance_porcelain::api::ApiCatalog { routes })
                );
                if matching > limit.unwrap_or(50) || (filter.is_none() && matching < total) {
                    println!("Use --filter <text> or --limit <number> to see more.");
                }
                println!("Use --format json for all schemas.");
            }
        }
        Ok(ApiOutcome::Invoked(value)) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(error) => anyhow::bail!("{}", serde_json::to_string(&error.failure)?),
    }
    Ok(())
}
