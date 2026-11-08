use super::OutputFormat;
use provenance_porcelain::api::{render_discovery_readable, ApiOutcome, ApiRequest};
use provenance_transport::repository::RepoContext;

/// Runs one API action with the selected context and applies discovery filters
/// and limits before rendering.
#[provenance_macros::rule("rule_porcelain_api_uses_context")]
#[provenance_macros::rule("rule_porcelain_output_reports_bounds")]
pub async fn dispatch_api(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    request: ApiRequest,
    filter: Option<&str>,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    let host = super::local_host(&RepoContext::new(repo, scope))?;
    let service = host.porcelain().api();
    match service.execute_api(request).await {
        Ok(ApiOutcome::Catalog(catalog)) => {
            let selection = catalog.select(filter, limit);
            let (total, matching, mut routes) =
                (selection.total, selection.matching, selection.routes);
            if format == Some(OutputFormat::Json) {
                let returned = routes.len();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "routes": routes,
                        "bounds": {
                            "total": total,
                            "matching": matching,
                            "returned": returned,
                            "truncated": returned < matching
                        }
                    }))?
                );
            } else {
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
        Err(error) => {
            let failure = serde_json::to_string(&error.failure)?;
            // Text mode prints the native cause of a failure of no known kind
            // as the cause of the refusal; the refusal object stays the same.
            return Err(match error.cause {
                Some(cause) if format != Some(OutputFormat::Json) => {
                    anyhow::Error::msg(cause).context(failure)
                }
                _ => anyhow::Error::msg(failure),
            });
        }
    }
    Ok(())
}
