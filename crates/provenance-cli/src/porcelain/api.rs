use super::OutputFormat;
use provenance_porcelain::api::{render_discovery_readable, ApiOutcome, ApiRequest, SeeMore};
use provenance_transport::repository::RepoContext;

/// The CLI flags that select more catalog routes.
const SEE_MORE: SeeMore = SeeMore {
    filter: "--filter <text>",
    limit: "--limit <number>",
};

/// Runs one API action with the selected context. JSON gives each selected route with
/// the bounds; text gives the bounded summary.
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
            println!("Use --format json for all schemas.");
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
