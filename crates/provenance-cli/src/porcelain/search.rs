use super::OutputFormat;
use provenance_porcelain::search::SearchInput;
use provenance_transport::repository::RepoContext;

/// Run one root search through the canonical typed operation.
pub async fn dispatch_search(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    query: SearchInput,
) -> anyhow::Result<()> {
    let host = super::local_host(&RepoContext::new(repo, scope))?;
    let service = host.porcelain().search();
    let outcome = service.search(query).await.map_err(|error| {
        anyhow::anyhow!(super::recovery::with_recovery(
            &error.to_string(),
            "search",
            &error.recovery_inputs(),
        ))
    })?;
    let rendered = if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(&outcome)?
    } else {
        provenance_porcelain::search::render_readable(&outcome)
    };
    println!("{rendered}");
    Ok(())
}
