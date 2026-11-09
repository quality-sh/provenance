use super::OutputFormat;
use provenance_porcelain::api::{render_discovery_readable, ApiOutcome, ApiRequest, SeeMore};
use provenance_transport::repository::RepoContext;

/// The CLI flags that select more catalog routes.
const SEE_MORE: SeeMore = SeeMore {
    filter: "--filter <text>",
    limit: "--limit <number>",
};

/// How to see the schemas of one route.
const SCHEMAS: &str =
    "Use --filter \"<method> <path>\" --format json to see the schemas of one route.";

/// Runs one API action with the selected context. JSON gives each selected route with
/// the bounds, and the schemas when it gives one route; text gives the bounded summary.
#[provenance_macros::rule("rule_porcelain_api_uses_context")]
pub async fn dispatch_api(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    request: ApiRequest,
) -> anyhow::Result<()> {
    let host = super::local_host(&RepoContext::new(repo, scope))?;
    let service = host.porcelain().api();
    match service.execute_api(request).await {
        Ok(ApiOutcome::Catalog(selection)) if format == Some(OutputFormat::Json) => {
            // Through a JSON value, so that the keys print in sorted order.
            let answer = serde_json::to_value(selection.complete())?;
            println!("{}", serde_json::to_string_pretty(&answer)?);
        }
        Ok(ApiOutcome::Catalog(selection)) => {
            println!(
                "{}",
                render_discovery_readable(&selection.bounded(), SEE_MORE)
            );
            println!("{SCHEMAS}");
        }
        Ok(ApiOutcome::Invoked(value)) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(error) => anyhow::bail!("{}", serde_json::to_string(&error.failure)?),
    }
    Ok(())
}
